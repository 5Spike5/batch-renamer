use slint::{ComponentHandle, Model};

use crate::App;



pub fn bind_check(app:&App) {
    let app_weak = app.as_weak();

    app.on_item_checked_changed(move |idx: i32,checked: bool|{
        let Some(app) = app_weak.upgrade()else {
            return;
        };

        let model = app.get_files();

        //idx是来自前端是i32，可能是负数或越界值，先安全转换并做边界检查
        let idx = match usize::try_from(idx) {
            Ok(i) if i < model.row_count() => i,
            _ => return,//非法索引直接忽略，避免后面panic
        };

        //row_data 返回的是这一行数据的拷贝（FileEntry实现了clone），
        //不是引用，所以改完之后必须set_row_data写回去才会生效
        if let Some(mut entry) = model.row_data(idx) {
            entry.checked = checked;
            model.set_row_data(idx, entry);
        }
    });
}