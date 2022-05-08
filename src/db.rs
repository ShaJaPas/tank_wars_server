use diesel::PgConnection;

lazy_static::lazy_static!{
    pub static ref POOL : thread_local::ThreadLocal<PgConnection> = {
        let tl = thread_local::ThreadLocal::with_capacity(4);
        tl
    };
}


