use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    path::PathBuf,
};

use crate::bencoding::torrent;

struct Target {
    file: File,
    start: u64,
    length: u64,
    tmp_path: PathBuf,
    final_path: PathBuf,
    renamed: bool,
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
        let first = self
            .targets
            .partition_point(|t| t.start + t.length <= piece_start);

        for target in &mut self.targets[first..] {
            if target.start >= piece_end {
                break;
            }
            let from = piece_start.max(target.start);
            let to = piece_end.min(target.start + target.length);
            if from >= to {
                continue;
            }
            target.file.seek(std::io::SeekFrom::Start(from - target.start))?;
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
                tmp_path,
                final_path,
                renamed,
                ..
            } = target;
            drop(file);
            if !renamed {
                fs::rename(tmp_path, final_path)?;
            }
        }
        fs::remove_file(&self.journal_path)?;
        self.completed = true;

        println!("File saved successfully!");
        Ok(())
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

    let (file, renamed) = if tmp_path.exists() {
        (OpenOptions::new().read(true).write(true).open(&tmp_path)?, false)
    } else if all_written && final_path.exists() {
        (OpenOptions::new().read(true).write(true).open(&final_path)?, true)
    } else {
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let new_file = File::create(&tmp_path)?;
        new_file.set_len(length)?;
        (new_file, false)
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
