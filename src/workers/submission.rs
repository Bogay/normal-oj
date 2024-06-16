use std::{fs::File, io::Write, path::PathBuf, process::Command};

use eyre::eyre;
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::models::{
    problems,
    submissions::{self, JudgeResult, Language},
};

/// Response to execute submissions
pub struct SubmissionWorker {
    pub ctx: AppContext,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct SubmissionWorkerArgs {
    /// ID of submissions this work need to process
    pub submission_id: i32,
}

impl worker::AppWorker<SubmissionWorkerArgs> for SubmissionWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }
}

impl SubmissionWorker {
    fn preprocess(s: &str) -> impl Iterator<Item = &str> {
        s.lines()
            .map(|l| l.trim_end())
            // reverse because we need to strip trailing newlines
            .rev()
            .skip_while(|l| l.is_empty())
    }

    /// Non-strict check whther two outputs are identical.
    pub fn compare_output(expected: &str, actual: &str) -> bool {
        Self::preprocess(expected).eq(Self::preprocess(actual))
    }
}

#[async_trait]
impl worker::Worker<SubmissionWorkerArgs> for SubmissionWorker {
    async fn perform(&self, args: SubmissionWorkerArgs) -> worker::Result<()> {
        let db = &self.ctx.db;

        // get submission && problem
        let subm = submissions::Model::find_by_id(db, args.submission_id)
            .await
            .map_err(Box::from)?;
        let problem = problems::Model::find_by_id(db, subm.problem_id)
            .await
            .map_err(Box::from)?;
        let tasks = problem.tasks(db).await.map_err(Box::from)?;

        let submission_dir = tempfile::tempdir()
            .map_err(|e| Box::from(eyre!("failed to create submission dir: {e}")))?;
        // extarct submission source
        let source_path = match subm.language {
            Language::C => submission_dir.path().join("main.c"),
            Language::Cpp => submission_dir.path().join("main.cpp"),
            Language::Python => submission_dir.path().join("main.py"),
        };
        let mut source_file = File::create(source_path.as_path())
            .map_err(|e| Box::from(eyre!("failed to create source code: {e}")))?;
        source_file
            .write_all(subm.code.as_bytes())
            .map_err(|e| Box::from(eyre!("failed to write source code: {e}")))?;
        // compile submission if needed
        let dup_judge_results = |r: &JudgeResult, p: &[problems::tasks::Model]| {
            let mut all = vec![];
            for (i, t) in p.iter().enumerate() {
                let mut seg = vec![];
                for j in 0..t.test_case_count {
                    let mut rs = r.clone();
                    rs.task_id = i as i32;
                    rs.case_id = j;
                    seg.push(rs);
                }
                all.push(seg);
            }
            all
        };
        match subm.language {
            Language::C => {
                let output = Command::new("gcc")
                    .args([
                        "-DONLINE_JUDGE",
                        "-O2",
                        "-w",
                        "-fmax-errors=3",
                        "-std=c11",
                        "main.c",
                        "-lm",
                        "-o",
                        "main",
                    ])
                    .current_dir(&submission_dir)
                    .output()
                    .map_err(|e| Box::from(eyre!("failed to compile c submission: {e}")))?;

                if !output.status.success() {
                    // dup CE result
                    let result: JudgeResult = JudgeResult {
                        status: "CE".to_string(),
                        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                        task_id: 0,
                        case_id: 0,
                        // TODO: use real data
                        duration: 1000,
                        mem_usage: 32768,
                    };
                    let all_results = dup_judge_results(&result, &tasks);
                    subm.into_active_model()
                        .update_sandbox_result(db, &problem, all_results)
                        .await
                        .map_err(Box::from)?;
                    return Ok(());
                }
            }
            Language::Cpp => {
                let output = Command::new("g++")
                    .args([
                        "-DONLINE_JUDGE",
                        "-O2",
                        "-w",
                        "-fmax-errors=3",
                        "-std=c++17",
                        "main.cpp",
                        "-lm",
                        "-o",
                        "main",
                    ])
                    .current_dir(&submission_dir)
                    .output()
                    .map_err(|e| Box::from(eyre!("failed to compile c++ submission: {e}")))?;
                if !output.status.success() {
                    // dup CE result
                    let result = JudgeResult {
                        status: "CE".to_string(),
                        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                        task_id: 0,
                        case_id: 0,
                        // TODO: use real data
                        duration: 1000,
                        mem_usage: 32768,
                    };
                    let all_results = dup_judge_results(&result, &tasks);
                    subm.into_active_model()
                        .update_sandbox_result(db, &problem, all_results)
                        .await
                        .map_err(Box::from)?;
                    return Ok(());
                }
            }
            Language::Python => {}
        }

        // check problems test case
        let problem_dir = PathBuf::from("problem").join(problem.id.to_string());
        if !problem_dir.exists() {
            // get test case binary and unzip it
            let Some(test_case_path) = problem.test_case_path() else {
                return Err(Box::from(problems::Error::NoTestCase))?;
            };
            let test_case: Vec<u8> = self
                .ctx
                .storage
                .download(test_case_path.as_path())
                .await
                .map_err(|e| Box::from(eyre!("failed to download test case: {e}")))?;

            let cursor = std::io::Cursor::new(test_case);
            let mut problem_zip = zip::ZipArchive::new(cursor)
                .map_err(|e| problems::Error::BadTestCase(problems::BadTestCase::ZipError(e)))
                .map_err(Box::from)?;
            problem_zip
                .extract(problem_dir.as_path())
                .map_err(|e| Box::from(eyre!("failed to extract problem zip: {e}")))?;

            tracing::info!(
                problem_id = problem.id,
                problem_name = problem.name,
                test_case_id = problem.test_case_id,
                "extract problem test case"
            );

            // TODO: validate directory structure?
        }
        let problem_dir = problem_dir.canonicalize().map_err(Box::from)?;

        // collect judge results
        let mut all_judge_results: Vec<Vec<JudgeResult>> = vec![];
        for (i, task) in tasks.iter().enumerate() {
            let mut task_results = vec![];
            for j in 0..task.test_case_count {
                let case_id = format!("{i:02}{j:02}");
                let stdin_path = problem_dir.join("test-case").join(&case_id).join("STDIN");
                let answer_path = problem_dir.join("test-case").join(&case_id).join("STDOUT");
                let stdin_str = stdin_path.to_string_lossy();
                let output_dir = tempfile::tempdir()
                    .map_err(|e| Box::from(eyre!("failed to create output dir: {e}")))?;
                let stdout_path = output_dir.path().join("stdout");
                let stderr_path = output_dir.path().join("stderr");
                let output_path = output_dir.path().join("output");
                let stdout_str = stdout_path.to_string_lossy();
                let stderr_str = stderr_path.to_string_lossy();
                let output_str = output_path.to_string_lossy();
                let time_limit = task.time_limit;
                let memory_limit = task.memory_limit;
                let lang: i32 = subm.language.clone().into();

                // save config to disk
                let config = toml::toml! {
                    cwd = "."
                    large-stack = true
                    max-process = 10
                    memory-limit = memory_limit
                    output-size-limit = 10000
                    runtime-limit = time_limit
                    lang = lang
                    stdin = stdin_str
                    stdout = stdout_str
                    stderr = stderr_str
                    output = output_str
                };
                let config_path = output_dir.path().join("noj.toml");
                let mut config_file = File::create(&config_path).map_err(Box::from)?;
                config_file
                    .write_all(
                        &toml::to_string(&config)
                            .map(|s| s.into_bytes())
                            .map_err(Box::from)?,
                    )
                    .map_err(Box::from)?;

                // invoke sandbox process
                // TODO: configurable sandbox path
                let sandbox_output = Command::new("sandbox")
                    .args(["--env-path", &config_path.to_string_lossy().to_string()])
                    .current_dir(&submission_dir)
                    .output();
                match sandbox_output {
                    Ok(o) if !o.status.success() => {
                        task_results.push(JudgeResult {
                            status: "JE".to_string(), // judge error
                            duration: -1,
                            mem_usage: -1,
                            stdout: String::from_utf8_lossy(&o.stdout).to_string(),
                            stderr: String::from_utf8_lossy(&o.stderr).to_string(),
                            task_id: 0,
                            case_id: 0,
                        })
                    }
                    Ok(_) => {
                        let sandbox_status_raw_string = std::fs::read_to_string(&output_path)
                            .map_err(|e| {
                                Box::from(eyre!(
                                    "failed to read sandbox result @{}: {}",
                                    output_path.display(),
                                    e
                                ))
                            })?;
                        let sandbox_status_raw_string =
                            sandbox_status_raw_string.lines().collect::<Vec<_>>();

                        let duration_ms: i32 =
                            sandbox_status_raw_string[2].parse().map_err(Box::from)?;
                        let mem_usage_kb: i32 =
                            sandbox_status_raw_string[3].parse().map_err(Box::from)?;
                        let _exit_msg = sandbox_status_raw_string[1].to_string();
                        let stdout = std::fs::read_to_string(stdout_path)
                            .map_err(|e| Box::from(eyre!("failed to read stdout: {e}")))?;
                        let stderr = std::fs::read_to_string(stderr_path)
                            .map_err(|e| Box::from(eyre!("failed to read stderr: {e}")))?;
                        let status = {
                            let s = sandbox_status_raw_string[0].to_string();
                            let answer = std::fs::read_to_string(answer_path)
                                .map_err(|e| Box::from(eyre!("failed to read answer: {e}")))?;

                            match s.as_str() {
                                "TLE" | "MLE" | "RE" | "OLE" => s,
                                _ => {
                                    if Self::compare_output(&answer, &stdout) {
                                        "AC".to_string()
                                    } else {
                                        "WA".to_string()
                                    }
                                }
                            }
                        };

                        task_results.push(JudgeResult {
                            status,
                            duration: duration_ms,
                            mem_usage: mem_usage_kb,
                            stdout,
                            stderr,
                            task_id: i as i32,
                            case_id: j,
                        })
                    }
                    Err(e) => {
                        task_results.push(JudgeResult {
                            status: "JE".to_string(), // judge error
                            duration: -1,
                            mem_usage: -1,
                            stdout: String::new(),
                            stderr: e.to_string(),
                            task_id: i as i32,
                            case_id: j,
                        })
                    }
                }
            }
            all_judge_results.push(task_results);
        }

        // upload judge result
        subm.into_active_model()
            .update_sandbox_result(db, &problem, all_judge_results)
            .await
            .map_err(Box::from)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::workers::submission::SubmissionWorker;

    #[test]
    fn test_compare_output() {
        let test_case = [
            // exactly the same
            ("aaa\nbbb\n", "aaa\nbbb\n", true),
            // trailing space before new line
            ("aaa  \nbbb\n", "aaa\nbbb\n", true),
            // redundant new line at the end
            ("aaa\nbbb\n\n", "aaa\nbbb\n", true),
            // redundant new line in the middle
            ("aaa\n\nbbb\n", "aaa\nbbb\n", false),
            // trailing space at the start
            ("aaa\n bbb", "aaa\nbbb\n", false),
            // empty string
            ("", "", true),
            // only new line
            ("\n\n\n\n", "", true),
            // empty character
            ("\t\r\n", "", true),
            // crlf
            ("crlf\r\n", "crlf\n", true),
        ];

        for (a, b, result) in test_case {
            assert!(SubmissionWorker::compare_output(a, b) == result);
        }
    }
}
