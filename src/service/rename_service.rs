use std::fs;
use std::path::Path;
use std::sync::mpsc;

use crate::model::rename_event::RenameEvent;

fn scan_directory(path : &str) -> Result<Vec<String>,String>{
    let path = Path::new(path);
    let entries = fs::read_dir(path).map_err(|e|format!("读取目录失败: {}", e))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e|format!("读取文件失败: {}", e))?;
        if !entry.path().is_file() {
            continue;
        }
        
        files.push(entry.file_name().to_string_lossy().into_owned());
    }
    files.sort();
    Ok(files)
}

fn generate_new_name(old_name:&str,rule:&str,index:usize,prefix:&str) -> String{
    let path = Path::new(old_name);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let ext = path.extension().unwrap_or_default().to_string_lossy().into_owned();
    let body = expand_placeholders(rule, index, prefix);
    if ext.is_empty() {
        body
    }else {
        format!("{}.{}",body,ext)
    }
}

fn expand_placeholders(rule:&str,index:usize,prefix:&str) -> String{
    todo!()
}