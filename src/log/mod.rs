use chrono::Utc;
use path_clean::clean;
use lazy_static::lazy_static;
use std::fs::{OpenOptions, File};
use std::{sync::Mutex, fs, io::Write};

lazy_static! {
    pub static ref BUFF: Mutex<String> = Mutex::new(String::new());
    pub static ref DIR_PREFIX: Mutex<Option<String>> = Mutex::new(Some("./Logs/".to_string()));
    pub static ref LOG_TAG: Mutex<Option<String>> = Mutex::new(None);
    pub static ref FILE_WRITTER: Mutex<Option<File>> = Mutex::new(None);
}

#[macro_export]
macro_rules! add_log {
    ($($arg:tt)*) => {{
        use chrono::Utc;
        use crate::log::BUFF;
        *BUFF.lock().unwrap() = format!("{}[{}] {}\n", 
            *BUFF.lock().unwrap(),
            Utc::now().format("%Y-%m-%d %H:%M:%S %Z").to_string(),
            format!($($arg)*)
        );
    }};
}

#[macro_export]
macro_rules! write_log {
    ($($arg:tt)*) => {{
        use chrono::Utc;
        use crate::log::{BUFF, flush_write};
        *BUFF.lock().unwrap() = format!("{}[{}] {}\n", 
            *BUFF.lock().unwrap(),
            Utc::now().format("%Y-%m-%d %H:%M:%S %Z").to_string(),
            format!($($arg)*)
        );
        flush_write();
    }};
}

pub fn set_tag(tag: &str) {
    *LOG_TAG.lock().unwrap() = Some(tag.to_string());
}

// write BUFF to a local file 
pub fn flush_write() {
    if FILE_WRITTER.lock().unwrap().is_some() {
        let mut writter_guard = FILE_WRITTER.lock().unwrap();
        let writter = writter_guard.as_mut().unwrap();
        write!(writter, "{}", *BUFF.lock().unwrap()).unwrap();
        *BUFF.lock().unwrap() = String::new(); 
    } else {
        if let Some(ref dir_prefix) = *DIR_PREFIX.lock().unwrap() {
            let log_tag = if LOG_TAG.lock().unwrap().is_some() {
                format!("-{}", LOG_TAG.lock().unwrap().as_ref().unwrap())
            } else { String::new() };
            let full_path = clean(format!(
                "{}/{}{}.log", 
                dir_prefix, 
                Utc::now().format("%Y%m%d_%H%M%S").to_string(), 
                log_tag
            ));
            if let (Ok(()), Ok(mut file_writter)) = (
                fs::create_dir_all(&dir_prefix), 
                OpenOptions::new().create(true).append(true).open(&full_path)
            ) {
                println!("[{}] Log updated.\n", 
                    Utc::now().format("%Y-%m-%d %H:%M:%S %Z").to_string()
                );
                write!(file_writter, "{}", *BUFF.lock().unwrap()).unwrap();
                *FILE_WRITTER.lock().unwrap() = Some(file_writter);
                *BUFF.lock().unwrap() = String::new();
                // file.write_all(BUFF.lock().unwrap().as_bytes()).unwrap();
            } else {
                println!(
                    "[{}] Log update failed.\n => check provided log directory prefix. [{}]", 
                    Utc::now().format("%Y-%m-%d %H:%M:%S %Z").to_string(),
                    dir_prefix
                );
            }
        }
    }
}

pub fn wipe_log() {
    *BUFF.lock().unwrap() = String::new();
}
