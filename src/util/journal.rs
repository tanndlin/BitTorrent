use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    path::PathBuf,
};

use sha1::{Digest, Sha1};

use crate::bencoding::torrent;

/// Largest block a peer may request; the spec allows rejecting anything over 16 KiB
/// but some clients ask for more, so allow up to 128 KiB
const MAX_BLOCK_LENGTH: u32 = 128 * 1024;

struct Target {
    file: File,
    start: u64,
    length: u64,
    tmp_path: PathBuf,
    final_path: PathBuf,
    renamed: bool,
    /// Opened from a final file that already existed, so its contents are unverified
    adopted: bool,
}

pub struct Journal {
    targets: Vec<Target>,
    piece_size: usize,
    pieces_written: Vec<bool>,
    num_pieces_written: u32,
    journal_file: Option<File>,
    journal_path: String,
    total_pieces: u32,
    completed: bool,
}

impl Journal {
    pub fn new(
        file_path: &str,
        file_size: usize,
        piece_size: usize,
        files: Vec<torrent::File>,
        piece_hashes: &[[u8; 20]],
    ) -> std::io::Result<Self> {
        let total_pieces = file_size.div_ceil(piece_size);

        let journal_path = &format!("{}{}", file_path, ".journal");
        let mut journal_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(journal_path)?;

        // Tolerate a short or missing journal: anything not recorded is treated as not written
        let mut buffer = Vec::new();
        journal_file.read_to_end(&mut buffer)?;
        buffer.resize(total_pieces, 0);
        let pieces_written: Vec<bool> = buffer.iter().map(|&byte| byte != 0).collect();
        let num_pieces_written = pieces_written.iter().filter(|v| **v).count() as u32;
        let all_written = num_pieces_written as usize == total_pieces;

        let layout = if files.is_empty() {
            vec![(PathBuf::from(file_path), file_size as u64)]
        } else {
            files
                .iter()
                .map(|file| Ok((safe_path(file_path, &file.path)?, file.length as u64)))
                .collect::<std::io::Result<Vec<_>>>()?
        };

        let mut targets = Vec::with_capacity(layout.len());
        let mut start = 0;
        for (final_path, length) in layout {
            targets.push(open_target(final_path, start, length, all_written)?);
            start += length;
        }

        if start != file_size as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "File lengths do not add up to the torrent length",
            ));
        }

        let total_pieces = total_pieces as u32;
        let mut journal = Journal {
            targets,
            piece_size,
            pieces_written,
            num_pieces_written,
            journal_file: Some(journal_file),
            journal_path: journal_path.to_string(),
            total_pieces,
            completed: false,
        };

        // Files that already exist (e.g. from another client) aren't covered by the
        // journal, so hash their pieces to find out what we already have
        if !all_written && journal.targets.iter().any(|t| t.adopted) {
            journal.verify_adopted_pieces(piece_hashes)?;
        }

        // A previous run may have written every piece without finalizing
        if journal.num_pieces_written == journal.total_pieces {
            journal.finalize()?;
        }

        Ok(journal)
    }

    pub fn write_piece(&mut self, piece_index: u32, data: &[u8]) -> std::io::Result<()> {
        if data.len() != self.piece_size && piece_index != self.total_pieces - 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Data length does not match piece size",
            ));
        }

        let Some(journal_file) = &mut self.journal_file else {
            // Already finalized, nothing left to write
            return Ok(());
        };

        let piece_start = piece_index as u64 * self.piece_size as u64;
        let piece_end = piece_start + data.len() as u64;
        let overlapping = overlapping_targets(&self.targets, piece_start, piece_end);
        for target in &mut self.targets[overlapping] {
            let from = piece_start.max(target.start);
            let to = piece_end.min(target.start + target.length);
            if from >= to {
                continue;
            }
            target
                .file
                .seek(std::io::SeekFrom::Start(from - target.start))?;
            target
                .file
                .write_all(&data[(from - piece_start) as usize..(to - piece_start) as usize])?;
        }

        // Update the journal file
        journal_file.seek(std::io::SeekFrom::Start(piece_index as u64))?;
        journal_file.write_all(&[1])?;
        if !std::mem::replace(&mut self.pieces_written[piece_index as usize], true) {
            self.num_pieces_written += 1;
        }

        if self.num_pieces_written == self.total_pieces {
            self.finalize()?;
        }

        Ok(())
    }

    /// Rehashes every piece that overlaps an adopted file and records whether it matches
    fn verify_adopted_pieces(&mut self, piece_hashes: &[[u8; 20]]) -> std::io::Result<()> {
        let file_size = self.targets.last().map_or(0, |t| t.start + t.length);
        let mut buffer = vec![0u8; self.piece_size];
        let mut verified = 0;
        let mut checked = 0;

        println!("Verifying existing files...");
        for piece_index in 0..self.total_pieces {
            let piece_start = piece_index as u64 * self.piece_size as u64;
            let piece_end = (piece_start + self.piece_size as u64).min(file_size);
            let overlapping = overlapping_targets(&self.targets, piece_start, piece_end);
            if !self.targets[overlapping.clone()].iter().any(|t| t.adopted) {
                continue;
            }

            let data = &mut buffer[..(piece_end - piece_start) as usize];
            read_range(&mut self.targets, piece_start, data)?;

            let hash: [u8; 20] = Sha1::digest(&*data).into();
            let valid = piece_hashes.get(piece_index as usize) == Some(&hash);
            let was_written =
                std::mem::replace(&mut self.pieces_written[piece_index as usize], valid);
            match (was_written, valid) {
                (false, true) => self.num_pieces_written += 1,
                (true, false) => self.num_pieces_written -= 1,
                _ => {}
            }
            checked += 1;
            verified += valid as u32;
        }
        println!("Verified {verified}/{checked} pieces from existing files");

        if let Some(journal_file) = &mut self.journal_file {
            let bytes: Vec<u8> = self.pieces_written.iter().map(|&w| w as u8).collect();
            journal_file.seek(std::io::SeekFrom::Start(0))?;
            journal_file.write_all(&bytes)?;
        }
        Ok(())
    }

    /// Closes the file handles, moves the temp files to their final paths and deletes the journal
    fn finalize(&mut self) -> std::io::Result<()> {
        for target in &self.targets {
            target.file.sync_all()?;
        }
        // Windows refuses to rename or delete files with open handles, so close them first
        let targets = std::mem::take(&mut self.targets);
        self.journal_file = None;

        for target in targets {
            let Target {
                file,
                start,
                length,
                tmp_path,
                final_path,
                renamed,
                ..
            } = target;
            drop(file);
            if !renamed {
                fs::rename(&tmp_path, &final_path)?;
            }
            // Keep a read-only handle so completed pieces can still be seeded
            self.targets.push(Target {
                file: File::open(&final_path)?,
                start,
                length,
                tmp_path,
                final_path,
                renamed: true,
                adopted: false,
            });
        }
        fs::remove_file(&self.journal_path)?;
        self.completed = true;

        println!("File saved successfully!");
        Ok(())
    }

    /// Reads `length` bytes at offset `begin` within a piece that has been written
    pub fn get_block(
        &mut self,
        piece_index: u32,
        begin: u32,
        length: u32,
    ) -> std::io::Result<Vec<u8>> {
        if !self.is_written(piece_index) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Piece {piece_index} has not been written"),
            ));
        }

        let file_size = self.targets.last().map_or(0, |t| t.start + t.length);
        let piece_start = piece_index as u64 * self.piece_size as u64;
        let piece_length = (file_size - piece_start).min(self.piece_size as u64);
        if length == 0 || length > MAX_BLOCK_LENGTH || begin as u64 + length as u64 > piece_length {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Invalid block request: piece {piece_index}, begin {begin}, length {length}"
                ),
            ));
        }

        let mut data = vec![0u8; length as usize];
        read_range(&mut self.targets, piece_start + begin as u64, &mut data)?;
        Ok(data)
    }

    pub fn num_written_pieces(&self) -> u32 {
        self.num_pieces_written
    }

    pub fn is_written(&self, piece_index: u32) -> bool {
        self.pieces_written
            .get(piece_index as usize)
            .copied()
            .unwrap_or(false)
    }
}

/// Range of target indices whose byte span intersects `[start, end)`
fn overlapping_targets(targets: &[Target], start: u64, end: u64) -> std::ops::Range<usize> {
    let first = targets.partition_point(|t| t.start + t.length <= start);
    let last = targets.partition_point(|t| t.start < end);
    first..last.max(first)
}

/// Fills `buf` with the torrent bytes starting at absolute offset `start`, reading
/// across file boundaries
fn read_range(targets: &mut [Target], start: u64, buf: &mut [u8]) -> std::io::Result<()> {
    let end = start + buf.len() as u64;
    let overlapping = overlapping_targets(targets, start, end);
    for target in &mut targets[overlapping] {
        let from = start.max(target.start);
        let to = end.min(target.start + target.length);
        if from >= to {
            continue;
        }
        target
            .file
            .seek(std::io::SeekFrom::Start(from - target.start))?;
        target
            .file
            .read_exact(&mut buf[(from - start) as usize..(to - start) as usize])?;
    }
    Ok(())
}

fn safe_path(root: &str, components: &[String]) -> std::io::Result<PathBuf> {
    let mut path = PathBuf::from(root);
    for component in components {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains(['/', '\\', ':'])
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unsafe path in torrent: {components:?}"),
            ));
        }
        path.push(component);
    }
    Ok(path)
}

fn open_target(
    final_path: PathBuf,
    start: u64,
    length: u64,
    all_written: bool,
) -> std::io::Result<Target> {
    let mut tmp_path = final_path.clone().into_os_string();
    tmp_path.push(".tmp");
    let tmp_path = PathBuf::from(tmp_path);

    let final_len = fs::metadata(&final_path).ok().map(|m| m.len());
    let (file, renamed, adopted) = if tmp_path.exists() {
        let file = OpenOptions::new().read(true).write(true).open(&tmp_path)?;
        (file, false, false)
    } else if all_written && final_path.exists() {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&final_path)?;
        (file, true, false)
    } else if final_len == Some(length) {
        // Write into the existing file in place; its pieces get verified by hash
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&final_path)?;
        (file, true, true)
    } else {
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let new_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;
        new_file.set_len(length)?;
        (new_file, false, false)
    };

    if file.metadata()?.len() != length {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "Existing file size does not match expected size: {}",
                tmp_path.display()
            ),
        ));
    }

    Ok(Target {
        file,
        start,
        length,
        tmp_path,
        final_path,
        renamed,
        adopted,
    })
}

impl Drop for Journal {
    fn drop(&mut self) {
        // The journal file was deleted on completion, so there is nothing to persist
        if self.completed {
            return;
        }
        let Some(journal_file) = &mut self.journal_file else {
            return;
        };

        let mut buffer = vec![0u8; self.total_pieces as usize];
        for (index, &written) in self.pieces_written.iter().enumerate() {
            if let Some(byte) = buffer.get_mut(index) {
                *byte = written as u8;
            }
        }
        journal_file
            .seek(std::io::SeekFrom::Start(0))
            .expect("Failed to seek journal file");
        journal_file
            .write_all(&buffer)
            .expect("Failed to write journal file");
    }
}
