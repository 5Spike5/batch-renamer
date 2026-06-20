use slint::{ComponentHandle, Model, ModelRc, VecModel};

use crate::{App, service::rename_service};


pub fn bind_preview(app:&App) {
    let app_weak = app.as_weak();

    app.on_preview_clicked(move |rule|{
        
        let Some(app) = app_weak.upgrade()else {
            return;
        };
        //校验
        let rule = rule.trim();
        if rule.is_empty() {
            app.set_status_text(
                "请输入重命名规则".into()
            );
            return;
        }
        // 当前UI中的文件列表
        let model = app.get_files();
        let mut files = Vec::new();
        if model.row_count() == 0 {
            app.set_status_text(
                "请先选择文件夹".into()
            );
            return;
        }
        for i in 0..model.row_count() {
            if let Some(file) = model.row_data(i) {
                files.push(file);
            }
        }

        //重新计算预览名称
        for (i,entry) in files.iter_mut().enumerate() {
            if entry.checked {
                entry.new_name = rename_service::generate_new_name(&entry.old_name, &rule, i+1, "").into();
            }else {
                entry.new_name = entry.old_name.clone();
            }
        }

        //写回UI
        let model = VecModel::from(files);
        
        app.set_files(ModelRc::new(model));
    });
}