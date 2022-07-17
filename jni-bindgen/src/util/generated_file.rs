use crate::util::{Difference, *};

use std::fs::{self, *};
use std::io::{self, BufRead, BufReader, Cursor, ErrorKind};
use std::path::{Path};

const MARKER_COMMENT : &str = "WARNING:  This file was autogenerated by jni-bindgen.  Any changes to this file may be lost!!!";



pub fn write_generated(context: &emit_rust::Context, path: &impl AsRef<Path>, contents: &[u8]) -> io::Result<()> {
    let path = path.as_ref();
    let dir = path.parent().ok_or_else(|| io_data_error!("{:?} has no parent directory", path))?;
    let _ = create_dir_all(dir);

    match File::open(&path) {
        Ok(file) => {
            let mut original = BufReader::new(file);
            let mut first_line = String::new();
            read_line_no_eol(&mut original, &mut first_line)?;

            let mut found_marker = false;
            for prefix in &["// ", "# "] {
                if first_line.starts_with(prefix) && (&first_line[prefix.len()..] == MARKER_COMMENT) {
                    found_marker = true;
                    break;
                }
            }

            if !found_marker {
                return io_data_err!("Cannot overwrite {:?}:  File exists, and first line {:?} doesn't match expected MARKER_COMMENT {:?}", path, first_line, MARKER_COMMENT);
            }

            let difference = Difference::find(&mut original, &mut Cursor::new(contents))?;
            match difference {
                None => {
                    context.progress.lock().unwrap().update(format!("unchanged: {}...", path.display()).as_str());
                    return Ok(());
                },
                Some(_difference) => {
                    context.progress.lock().unwrap().force_update(format!("MODIFIED: {}", path.display()).as_str());
                },
            }
        },
        Err(ref e) if e.kind() == ErrorKind::NotFound => {
            context.progress.lock().unwrap().force_update(format!("NEW: {}", path.display()).as_str());
        },
        Err(e) => { return Err(e); },
    };

    fs::write(path, contents)
}



fn read_line_no_eol(reader: &mut impl BufRead, buffer: &mut String) -> io::Result<usize> {
    let size = reader.read_line(buffer)?;
    while buffer.ends_with('\r') || buffer.ends_with('\n') {
        buffer.pop();
    }
    Ok(size)
}
