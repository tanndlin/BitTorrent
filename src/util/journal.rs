use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    path::Path,
};

pub struct Journal {
    file: File,
    piece_size: usize,
    pub pieces_written: HashMap<u32, bool>,
    journal_file: File,
    journal_path: String,
    final_file_path: String,
    total_pieces: u32,
    completed: bool,
}

impl Journal {
    pub fn new(file_path: &str, file_size: usize, piece_size: usize) -> std::io::Result<Self> {
        let total_pieces = file_size.div_ceil(piece_size);

        let temp_file_path = &format!("{file_path}.tmp");
        let file = if Path::new(temp_file_path).exists() {
            let existing_file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(temp_file_path)?;
            let metadata = existing_file.metadata()?;
            if metadata.len() as usize != file_size {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Existing file size does not match expected size",
                ));
            }
            existing_file
        } else {
            let new_file = File::create(temp_file_path)?;
            new_file.set_len(file_size as u64)?;
            new_file
        };

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
        let pieces_written = buffer
            .iter()
            .enumerate()
            .map(|(i, &byte)| (i as u32, byte != 0))
            .collect();

        let total_pieces = total_pieces as u32;
        Ok(Journal {
            file,
            piece_size,
            pieces_written,
            journal_file,
            journal_path: journal_path.to_string(),
            final_file_path: file_path.to_string(),
            total_pieces,
            completed: false,
        })
    }

    pub fn write_piece(&mut self, piece_index: u32, data: &[u8]) -> std::io::Result<()> {
        if data.len() != self.piece_size && piece_index != self.total_pieces - 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Data length does not match piece size",
            ));
        }

        let offset = piece_index as u64 * self.piece_size as u64;
        self.file.seek(std::io::SeekFrom::Start(offset))?;
        self.file.write_all(data)?;
        self.pieces_written.insert(piece_index, true);

        // Update the journal file
        self.journal_file
            .seek(std::io::SeekFrom::Start(piece_index as u64))?;
        self.journal_file.write_all(&[1])?;

        if self.num_written_pieces() == self.total_pieces {
            let tmp_path = format!("{}.tmp", self.final_file_path);
            fs::rename(tmp_path, &self.final_file_path).unwrap();
            fs::remove_file(&self.journal_path)?;
            self.completed = true;
            println!("File saved successfully!");
        }

        Ok(())
    }

    pub fn num_written_pieces(&self) -> u32 {
        self.pieces_written.iter().filter(|(_, v)| **v).count() as u32
    }
}

impl Drop for Journal {
    fn drop(&mut self) {
        // The journal file was deleted on completion, so there is nothing to persist
        if self.completed {
            return;
        }

        let mut buffer = vec![0u8; self.total_pieces as usize];
        for (&index, &written) in &self.pieces_written {
            if let Some(byte) = buffer.get_mut(index as usize) {
                *byte = written as u8;
            }
        }
        self.journal_file
            .seek(std::io::SeekFrom::Start(0))
            .expect("Failed to seek journal file");
        self.journal_file
            .write_all(&buffer)
            .expect("Failed to write journal file");
    }
}
