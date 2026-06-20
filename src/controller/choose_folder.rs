use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{App, FileEntry, service::rename_service};


pub fn bind_choose_folder(app:&App) {
    let app_weak = app.as_weak();

    app.on_choose_folder_clicked(move ||{
        let Some(app) = app_weak.upgrade()else {
            return;
        };

        //1.弹出对话框(用户选择目录)->(继续执行)->(用户取消)->(直接返回)
        let Some(folder_path) = rfd::FileDialog::new().pick_folder()
        else {
            return;
        };

        //2.PathBuf -> String
        let path_str = folder_path.to_string_lossy().to_string();

        //3.扫描目录
        let files = match rename_service::scan_directory(&path_str) {
            Ok(files) => files,
            Err(err) =>{
                app.set_status_text(format!("扫描失败：{}",err).into());
                return;
            }
        };

        //4.构造Vec<FileEntry>
        let mut entries = Vec::new();

        for (i,file_name) in files.iter().enumerate() {
            entries.push(
                FileEntry{
                    old_name:file_name.clone().into(),
                    // 初始预览=原文件名
                    new_name:file_name.clone().into(),
                    checked:true,
                    index:i as i32
                }
            );
        }

        // 5. Vec -> VecModel
        let model: VecModel<FileEntry> = VecModel::from(entries);

        //6.更新UI
        app.set_files(
            ModelRc::new(model)
        );

        app.set_folder_path(path_str.into());

        app.set_status_text(
            format!("发现 {} 个文件", files.len())
                .into()
        );

    });
}