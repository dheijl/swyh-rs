use fltk::app:: { awake_msg, thread_msg };

pub fn fltk_app_awake(msg: i32) {
    unsafe {
        awake_msg(msg);
    }
}

pub fn fltk_app_thread_msg() ->  i32 {
    if let Some(msg) = unsafe { thread_msg::<i32>() } {
        msg
    } else {
        -1
    }
}
