//! 集成测试：`docs/` 与根目录 Markdown 的内部一致性。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// 免检关键字。
const SKIP_MARKER: &str = "<!-- docs-links: skip -->";

/// 仓库根目录（`cargo test` 的工作目录不保证是它，所以用 `CARGO_MANIFEST_DIR` 定位）。
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 要检查的 Markdown：仓库根目录一层的 + `docs/` 下的（`CONTEXT-zh_cn.md` 在 `docs/CONTEXTS/`，递归会查到）。
///
/// `docs/` 递归下去：文档按用途分了子目录（`README/`、`tutorial/`、`mods/<模块>/`、`refs/` 等，见
/// `docs/index-*.md` 的说明），每份文档都带语言后缀（`-zh_cn` / `-en_us`，缺的那份是空占位）。
/// 子目录里的链接与锚点同样要盯；仓库根只取一层，免得走进 `target/`、`.git/` 这些不属于文档的目录。
fn markdown_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("读取仓库根目录 {} 失败: {error}", root.display()))
        .map(|entry| entry.expect("读取仓库根目录中的目录项失败"))
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
        .collect();

    collect_markdown(&root.join("docs"), &mut files);
    // 软链接（`docs/README/README-en_us.md` → 仓库根 `README.md`）只是同一份内容的另一个入口：
    // 它的相对链接按它自己所在目录解析，而内容里的链接是按目标文件的目录写的，在那里自然对不上。
    // 跳过它——内容已按目标位置查过，软链接本身由 `the_english_readme_is_a_symlink_to_the_default_readme` 盯着。
    files.retain(|path| !is_symlink(path));
    files.sort();

    files
}

/// 是不是软链接（`fs::metadata` 会跟着链接走，所以要用 `symlink_metadata`）。
fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

/// 递归收集一个目录下的全部 Markdown；读取失败时立即报错，避免静默漏检文档。
fn collect_markdown(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("读取 Markdown 目录 {} 失败: {error}", dir.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "读取 Markdown 目录 {} 中的目录项失败: {error}",
                dir.display()
            )
        });
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
}

/// 要检查的正文：带行号、跳过 ``` 代码块里的行（代码块里的 `# 注释` 不是标题，里面的路径也多是命令
/// 而不是本仓库的文件），并跳过标了免检的文件（返回空）。
fn checked_lines(markdown: &str) -> Vec<(usize, &str)> {
    if markdown
        .lines()
        .take(20)
        .any(|line| line.contains(SKIP_MARKER))
    {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut in_code_block = false;
    for (index, line) in markdown.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if !in_code_block {
            lines.push((index + 1, line));
        }
    }

    lines
}

/// 一个 Markdown 文件里的标题锚点（仅统计代码围栏外的 ATX 标题）。
fn heading_anchors(markdown: &str) -> BTreeSet<String> {
    checked_lines(markdown)
        .into_iter()
        .filter_map(|(_, line)| {
            let heading = line.trim_start();
            let marker_len = heading
                .chars()
                .take_while(|character| *character == '#')
                .count();

            // ATX 标题最多六个 #；标记后必须是空白或行尾，避免把 #comment 当作标题。
            if marker_len == 0 || marker_len > 6 {
                return None;
            }
            let remainder = &heading[marker_len..];
            if remainder
                .chars()
                .next()
                .is_some_and(|character| !character.is_whitespace())
            {
                return None;
            }

            Some(anchor_slug(
                remainder.trim().trim_end_matches('#').trim_end(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 代码示例中的 `#` 不应被误认为文档标题锚点。
    #[test]
    fn headings_inside_fenced_code_blocks_are_ignored() {
        let anchors = heading_anchors("# Real heading\n\n```markdown\n# Example only\n```\n");

        assert!(anchors.contains("real-heading"));
        assert!(!anchors.contains("example-only"));
    }

    /// 只识别 CommonMark ATX 标题形状，不把普通井号文本或七级标题当成标题。
    #[test]
    fn only_valid_atx_heading_markers_produce_anchors() {
        let anchors =
            heading_anchors("# Valid\n### Also valid ###\n#not-a-heading\n####### Too deep\n");

        assert!(anchors.contains("valid"));
        assert!(anchors.contains("also-valid"));
        assert!(!anchors.contains("not-a-heading"));
        assert!(!anchors.contains("too-deep"));
    }
}

/// 标题 → 锚点（照 GitHub：小写、去掉标点（`-` 与 `_` 除外）、空格换连字符）。
fn anchor_slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// 把行内链接拆出来：`[文字](目标)`，返回所有目标。
fn link_targets(line: &str) -> Vec<&str> {
    let mut targets = Vec::new();
    let mut rest = line;

    while let Some(open) = rest.find("](") {
        let after_open = &rest[open + 2..];
        let Some(close) = after_open.find(')') else {
            break;
        };
        let target = &after_open[..close];
        // 图片 `![x](y)` 也算同一类链接，这里的解析方式对它们一样适用。
        targets.push(target);
        rest = &after_open[close + 1..];
    }

    targets
}

/// 显示成相对仓库根的形式，失败信息里好读。
fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// 每一条指向仓库内文件的链接都要存在（含 `docs/x.md` 与 `x.md` 两种写法，相对链接所在目录解析）。
#[test]
fn every_doc_link_points_at_an_existing_file() {
    let root = repo_root();
    let mut problems = Vec::new();

    for path in markdown_files() {
        let markdown = fs::read_to_string(&path).expect("读 Markdown");
        let dir = path.parent().expect("文件都有父目录").to_path_buf();

        for (line_no, line) in checked_lines(&markdown) {
            for target in link_targets(line) {
                // 外链、邮件与纯锚点（`#xxx`）不在这里管。
                if target.starts_with("http")
                    || target.starts_with("mailto")
                    || target.starts_with('#')
                {
                    continue;
                }

                let file_part = target.split('#').next().unwrap_or_default();
                if file_part.is_empty() {
                    continue;
                }

                if !dir.join(file_part).exists() {
                    problems.push(format!(
                        "{}:{} -> {}（目标文件不存在）",
                        relative(&path, &root),
                        line_no,
                        target
                    ));
                }
            }
        }
    }

    assert!(
        problems.is_empty(),
        "内部链接有断链:\n{}",
        problems.join("\n")
    );
}

/// 每一条带锚点的链接都要指向目标文件里真实存在的标题。
#[test]
fn every_doc_anchor_matches_a_heading() {
    let root = repo_root();
    let mut problems = Vec::new();

    for path in markdown_files() {
        let markdown = fs::read_to_string(&path).expect("读 Markdown");
        let dir = path.parent().expect("文件都有父目录").to_path_buf();

        for (line_no, line) in checked_lines(&markdown) {
            for target in link_targets(line) {
                if target.starts_with("http") || target.starts_with("mailto") {
                    continue;
                }

                let Some((file_part, anchor)) = target.split_once('#') else {
                    continue;
                };
                if anchor.is_empty() {
                    continue;
                }

                // 纯锚点（`#xxx`）指本文，其余相对链接所在目录。
                let target_path = if file_part.is_empty() {
                    path.clone()
                } else {
                    dir.join(file_part)
                };
                let Ok(target_markdown) = fs::read_to_string(&target_path) else {
                    continue; // 目标文件不存在由上一个测试报，这里不重复。
                };

                if !heading_anchors(&target_markdown).contains(anchor) {
                    problems.push(format!(
                        "{}:{} -> {}（锚点对不上任何标题）",
                        relative(&path, &root),
                        line_no,
                        target
                    ));
                }
            }
        }
    }

    assert!(
        problems.is_empty(),
        "文档锚点对不上标题:\n{}",
        problems.join("\n")
    );
}

/// `docs/` 里写出来的代码路径（反引号里的 `src/....rs`）都要真实存在 —— 改名、拆文件之后最容易忘。
///
/// 只查"形状确定的自家路径"：以 `src/` 开头、以 `.rs` 结尾、不含通配符与省略号与花括号简写
/// （`src/text/*.rs`、`src/commands/{a,b}.rs` 这类跳过），而且它所在的目录必须真的在仓库里
/// （这样 `poise_macros` 的 `src/command/mod.rs` 这种引用别人 crate 内部路径的句子不会被误判）。
#[test]
fn every_source_path_mentioned_in_docs_exists() {
    let root = repo_root();
    let mut problems = Vec::new();

    for path in markdown_files() {
        let markdown = fs::read_to_string(&path).expect("读 Markdown");

        for (line_no, line) in checked_lines(&markdown) {
            for quoted in line.split('`').skip(1).step_by(2) {
                let candidate = quoted.trim();
                if !candidate.starts_with("src/") || !candidate.ends_with(".rs") {
                    continue;
                }
                if candidate.contains('*')
                    || candidate.contains('{')
                    || candidate.contains("…")
                    || candidate.contains("..")
                {
                    continue;
                }

                // 只认自家目录下的：父目录得在仓库里存在（否则是别的 crate 的路径）。
                let Some(parent) = Path::new(candidate).parent() else {
                    continue;
                };
                if !root.join(parent).is_dir() {
                    continue;
                }

                if !root.join(candidate).exists() {
                    problems.push(format!(
                        "{}:{} -> {}（代码路径不存在）",
                        relative(&path, &root),
                        line_no,
                        candidate
                    ));
                }
            }
        }
    }

    assert!(
        problems.is_empty(),
        "文档里提到的代码路径不存在:\n{}",
        problems.join("\n")
    );
}

/// `docs/README/README-en_us.md` 必须是指向仓库根 `README.md` 的软链接，不能是复制的一份。
///
/// 仓库根的 `README.md` 是默认 README（GitHub 首页与 crates.io 显示的就是它，链接按仓库根写）；
/// `docs/README/` 里那份是它在文档树里的入口，用软链接指回根 —— 一份内容两个位置，改一处不会忘记另一处。
#[test]
fn the_english_readme_is_a_symlink_to_the_default_readme() {
    let root = repo_root();
    let link = root.join("docs/README/README-en_us.md");

    let metadata = fs::symlink_metadata(&link).expect("docs/README/README-en_us.md 必须存在");
    assert!(
        metadata.file_type().is_symlink(),
        "docs/README/README-en_us.md 应当是指向 ../../README.md 的软链接（不要复制一份内容）"
    );

    let target = fs::read_link(&link).expect("读软链接");
    assert_eq!(
        target,
        Path::new("../../README.md"),
        "软链接要写成相对路径 ../../README.md，克隆到任何地方都有效"
    );
    assert!(link.is_file(), "软链接指向的 README.md 不存在");
}
