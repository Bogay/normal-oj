mod models;
mod requests;
mod tasks;
mod workers;

macro_rules! configure_insta {
    ($suffix:expr) => {
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_snapshot_suffix($suffix);
        let _guard = settings.bind_to_scope();
    };
}

pub(crate) use configure_insta;

// utils

use normal_oj::models::problems;
use sea_orm::ConnectionTrait;
use std::io::Write;
use zip::write::SimpleFileOptions;

async fn make_test_case<C: ConnectionTrait>(
    db: &C,
    problem: &problems::Model,
) -> zip::result::ZipResult<Vec<u8>> {
    let tasks = problem.tasks(db).await.unwrap();
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut test_case = zip::ZipWriter::new(&mut buf);
        let opt = SimpleFileOptions::default();
        test_case.add_directory("include/", opt)?;
        test_case.add_directory("share/", opt)?;
        for (task_i, task) in tasks.iter().enumerate() {
            for case_i in 0..task.test_case_count {
                let in_path = format!("test-case/{task_i:02}{case_i:02}/STDIN");
                test_case.start_file(in_path, opt)?;
                test_case.write_all(b"1 2\n")?;
                let out_path = format!("test-case/{task_i:02}{case_i:02}/STDOUT");
                test_case.start_file(out_path, opt)?;
                test_case.write_all(b"3\n")?;
            }
        }
    }
    Ok(buf.into_inner())
}
