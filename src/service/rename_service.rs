use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::mpsc::{self, Sender};

use chrono::Datelike;

use crate::FileEntry;
use crate::model::rename_event::RenameEvent;

pub fn scan_directory(path : &str) -> Result<Vec<String>,String>{
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

pub fn generate_new_name(old_name:&str,rule:&str,index:usize,prefix:&str) -> String{
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

pub fn expand_placeholders(rule:&str,index:usize,prefix:&str) -> String{
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let result = rule.replace("{n}", &format!("{:03}",index));
    let result = result.replace("{prefix}", prefix);
    let result = result.replace("{YYYYMMDD}", &today);
    result
}


pub fn execute_rename(entries:Vec<FileEntry>,folder:&str,tx:Sender<RenameEvent>) {
    let total = entries.len();
    let mut failed:usize = 0;
    let mut success:usize = 0;
    
    for (i,entry) in entries.iter().enumerate() {
        let current = i + 1;
        let old_path = Path::new(folder).join(&entry.old_name);
        let new_path = Path::new(folder).join(&entry.new_name);
        let current_file = entry.old_name.clone();
        
        if old_path == new_path {
            success += 1;

            let _ = tx.send(
            RenameEvent::Progress {
                current,
                total,
                current_file,
            }
        );
            continue;
        }
        if new_path.exists() {
            failed += 1;
            let _ = tx.send(
                RenameEvent::Error(format!("{}已存在",entry.new_name))
            );
            continue;
        }
       match  fs::rename(&old_path, &new_path) {
           Ok(_) => {success += 1;},
           Err(e) =>{
                failed += 1;

                let _ = tx.send(
                    RenameEvent::Error(format!("{} 重命名失败:{}",entry.old_name,e))
                );
           }
       }

        let _ = tx.send(
            RenameEvent::Progress {
                current,
                total,
                current_file,
            }
        );
    }

    let _ = tx.send(RenameEvent::Finished { success, failed });
}
/// 校验预览是否合法
///
/// 检查：
/// 1. 规则不能为空
/// 2. 至少勾选一个文件
/// 3. 文件名不能为空
/// 4. 文件名不能重复
/// 5. 文件名不能包含非法字符
pub fn validate_preview(files:&[FileEntry],rule:&str) -> Result<(),String>{
    //1.规则不能为空
    if rule.trim().is_empty() {
        return Err("请输入重命名规则".into());
    }
    // 2. 至少勾选一个文件
    if !files.iter().any(|f| f.checked) {
        return Err("请至少选择一个文件".into());
    }
    let mut names = HashSet::new();
    let mut errors = Vec::new();
    
    for file in files {
        if !file.checked {
            continue;
        }
        // 3. 文件名不能为空
        if file.new_name.trim().is_empty(){
            errors.push(format!(
            "[{}]：新文件名不能为空",
            file.old_name
            ));
        }
        
        // 4. 文件名不能重复
        if !names.insert(file.new_name.clone()) {
            errors.push(format!(
                "[{}]：与其它文件重名 ({})",
                file.old_name,
                file.new_name
            ));
        }
        // 5. 文件名不能包含非法字符
        if  contains_invalid_chars(&file.new_name){
            errors.push(format!(
                "[{}]：包含非法字符 ({})",
                file.old_name,
                file.new_name
            ));
        }
    }
    
    if errors.is_empty() {
        Ok(())
    }else {
        Err(errors.join("\n"))
    }
}

fn contains_invalid_chars(name:&str) -> bool {
    const INVALID : [char;9] = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    name.chars().any(|c| INVALID.contains(&c))
}