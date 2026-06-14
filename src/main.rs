slint::include_modules!();
mod model;
mod controller;
use crate::model::rename_event::RenameEvent;
fn main() ->Result<(),slint::PlatformError>{
    let app = App::new()?;
    

    //1.创建通道
    let (tx,rx) = std::sync::mpsc::channel::<RenameEvent>();
    // controller::app_controller::bind_events(app.clone(), tx);



    app.run()
}
