//! 集成测试：`docs/` 与根目录 Markdown 的内部一致性。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// 免检关键字。
const SKIP_MARKER: &str = "<!-- docs-links: skip -->";

/// 返回仓库根目录。
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 收集仓库根目录和 `docs/` 下的 Markdown。
fn markdown_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("读取仓库根目录 {} 失败: {error}", root.display()))
        .map(|entry| entry.expect("读取仓库根目录中的目录项失败"))
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
        .collect();

    collect_markdown(&root.join("docs"), &mut files);
    // 软链接的相对链接按目标文件的位置检查。
    files.retain(|path| !is_symlink(path));
    files.sort();

    files
}

/// 检查路径是否为软链接。
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

/// 返回待检查正文及行号；跳过代码块和免检文档。
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

    #[test]
    fn repository_root_links_resolve_from_any_document() {
        let root = repo_root();
        let expected = root.join("docs/assets/demo1.gif");

        for document in [
            root.join("README.md"),
            root.join("docs/README/README-zh_cn.md"),
        ] {
            assert_eq!(
                resolve_link_path(&root, &document, "/docs/assets/demo1.gif"),
                expected
            );
        }
        assert!(expected.is_file());
        assert!(
            !resolve_link_path(
                &root,
                &root.join("README.md"),
                "/docs/assets/demo1.gif/nonexistent"
            )
            .exists()
        );
    }

    #[test]
    fn relative_links_and_fragment_only_links_use_document_location() {
        let root = repo_root();
        let document = root.join("docs/README/README-zh_cn.md");

        assert_eq!(
            resolve_link_path(&root, &document, "../assets/demo1.gif"),
            root.join("docs/README/../assets/demo1.gif")
        );
        assert_eq!(resolve_link_path(&root, &document, ""), document);
    }

    #[test]
    fn repository_root_links_can_target_heading_anchors() {
        let root = repo_root();
        let document = root.join("docs/README/README-zh_cn.md");
        let target_path = resolve_link_path(&root, &document, "/README.md");
        let markdown = fs::read_to_string(target_path).expect("读取根 README");
        let anchors = heading_anchors(&markdown);

        assert!(anchors.contains("1-project-overview"));
        assert!(!anchors.contains("nonexistent-heading"));
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
        // 图片与普通链接使用相同的目标解析。
        targets.push(target);
        rest = &after_open[close + 1..];
    }

    targets
}

/// 仓库根路径从根目录解析，相对路径从文档所在目录解析，空路径指向文档本身。
fn resolve_link_path(root: &Path, document: &Path, file_part: &str) -> PathBuf {
    if file_part.is_empty() {
        document.to_path_buf()
    } else if file_part.starts_with('/') {
        root.join(file_part.trim_start_matches('/'))
    } else {
        document.parent().expect("文件都有父目录").join(file_part)
    }
}

/// 显示成相对仓库根的形式，失败信息里好读。
fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// 每一条指向仓库内文件的链接都要存在。
#[test]
fn every_doc_link_points_at_an_existing_file() {
    let root = repo_root();
    let mut problems = Vec::new();

    for path in markdown_files() {
        let markdown = fs::read_to_string(&path).expect("读 Markdown");
        for (line_no, line) in checked_lines(&markdown) {
            for target in link_targets(line) {
                // 跳过外链、邮件和纯锚点。
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

                if !resolve_link_path(&root, &path, file_part).exists() {
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

                let target_path = resolve_link_path(&root, &path, file_part);
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

/// 检查文档中完整的仓库源码路径；跳过通配符和外部 crate 路径。
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

                // 外部 crate 的源码路径不在仓库内校验。
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

/// 英文文档入口必须以相对软链接指向仓库根 README。
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
