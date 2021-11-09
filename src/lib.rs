mod error;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Read, Stdout, Write},
    ops::Range,
    path::Path,
};

use clap::Parser;
use colored::Colorize;
pub use error::GrepError;
use regex::Regex;

// 定义类型，简化书写
pub type StrategyFn<W, R> = fn(&Path, BufReader<R>, &Regex, &mut W) -> Result<(), GrepError>;

/// 简化版 grep ，支持正则表达式和文件通配符
#[derive(Debug, Parser)]
#[clap(version = "1.0", author = "Tyr Chen")]
pub struct GrepConfig {
    /// 用于查找的正则表达式
    pattern: String,
    /// 文件通配符
    glob: String,
}

impl GrepConfig {
    /// 使用缺省策略来查找匹配
    pub fn match_with_default_strategy(&self) -> Result<(), GrepError> {
        self.match_with(default_strategy)
    }
    /// 使用某个策略来查找匹配
    pub fn match_with(&self, strategy: StrategyFn<Stdout, File>) -> Result<(), GrepError> {
        let regex = Regex::new(&self.pattern)?;
        // 生成所有符合通配符的文件列表
        let files: Vec<_> = glob::glob(&self.glob)?.collect();
        // 并行处理所有文件
        files.into_par_iter().for_each(|v| {
            if let Ok(filename) = v {
                if let Ok(file) = File::open(&filename) {
                    let reader = BufReader::new(file);
                    let mut stdout = io::stdout();

                    if let Err(e) = strategy(filename.as_path(), reader, &regex, &mut stdout) {
                        println!("Internal error: {:?}", e);
                    }
                }
            }
        });
        Ok(())
    }
}

/// 缺省策略，从头到位串行查找，最后输出到 writer
pub fn default_strategy<W: Write, R: Read>(
    path: &Path,
    reader: BufReader<R>,
    pattern: &Regex,
    writer: &mut W,
) -> Result<(), GrepError> {
    let matches: String = reader
        .lines()
        .enumerate()
        .map(|(lineno, line)| {
            line.ok()
                .map(|line| {
                    pattern
                        .find(&line)
                        .map(|m| format_line(&line, lineno + 1, m.range()))
                })
                .flatten()
        })
        .filter_map(|v| v.ok_or(()).ok())
        .join("\n");
    if !matches.is_empty() {
        writer.write(path.display().to_string().green().as_bytes())?;
        writer.write(b"\n")?;
        writer.write(matches.as_bytes())?;
        writer.write(b"\n")?;
    }
    Ok(())
}

/// 格式化输出匹配的行，包括行号、列号和带有高亮的第一个匹配项
pub fn format_line(line: &str, lineno: usize, range: Range<usize>) -> String {
    let Range { start, end } = range;
    let prefix = &line[..start];
    format!(
        "{0: >6}:{1: <3} {2}{3}{4}",
        lineno.to_string().blue(),
        // 找到匹配项的起始位置，注意对汉字等非 ascii 字符，我们不能使用 prefix.len()
        // 这是一个 O(n) 的操作，会拖累效率，这里只是为了演示的效果
        (prefix.chars().count() + 1).to_string().cyan(),
        prefix,
        &line[start..end].red(),
        &line[end..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_line_should_workd() {
        let result = format_line("Hello, Tyr~", 1000, 7..10);
        let expected = format!(
            "{0: >6}:{1: <3} Hello, {2}~",
            "1000".blue(),
            "8".cyan(),
            "Tyr".red()
        );
        assert_eq!(result, expected);
        println!("{}", result);
    }

    #[test]
    fn default_strategy_should_work() {
        let path = Path::new("src/main.rs");
        let input = b"hello world!\nhey Tyr!";
        let reader = BufReader::new(&input[..]);
        let pattern = Regex::new(r"he\w+").unwrap();
        let mut writer = Vec::new();
        default_strategy(path, reader, &pattern, &mut writer).unwrap();
        let result = String::from_utf8(writer).unwrap();
        let expected = [
            String::from("src/main.rs"),
            format_line("hello world!", 1, 0..5),
            format_line("hey Tyr!\n", 2, 0..3),
        ];
        assert_eq!(result, expected.join("\n"))
    }
}
