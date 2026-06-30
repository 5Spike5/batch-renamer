use std::{sync::mpsc::Sender, thread};

use slint::{ComponentHandle, Model};

use crate::{App, FileEntry, model::rename_event::RenameEvent, service};



pub fn bind_execute(app:&App,tx:Sender<RenameEvent>) {
    let app_weak = app.as_weak();

    app.on_execute_clicked(move ||{
        let Some(app) = app_weak.upgrade()else {
            return;
        };
        // 1. 从 app.get_files() 读取当前文件列表
        let model = app.get_files();
        let count = model.row_count();
        let entries:Vec<FileEntry> = (0..count)
            .filter_map(|i| model.row_data(i))
            .collect();

        // 2. 过滤 checked == true 的条目
        let selected:Vec<FileEntry> = entries.into_iter()
            .filter(|e| e.checked)
            .collect();

        // 3. 如果没有选中文件，set_status_text("请勾选文件")后返回return
        if selected.is_empty() {
            app.set_status_text("请勾选文件".into());
            return;
        }

        // 4. 锁定 UI：app.set_is_running(true)（执行期间禁止重复点击）
        app.set_is_running(true);

        // 5. 获取文件夹路径
        let folder = app.get_folder_path().to_string();

        // 6. 开线程,克隆一份 tx 给新线程，原 tx 留给下一次点击复用
        let tx = tx.clone();
        thread::spawn(move ||{
            service::rename_service::execute_rename(selected, &folder, tx);
        });
    });
}