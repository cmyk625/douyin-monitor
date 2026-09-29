//! 端到端冒烟测试：真实启动浏览器 → CDP 建立会话 → 打开本地页面 → 执行解析脚本。
//! 未检测到 Chrome/Edge 时自动跳过。默认 `#[ignore]`，用
//! `cargo test --test cdp_smoke -- --ignored --nocapture` 运行。
use douyin_monitor_lib::collector::{self, Browser};
use douyin_monitor_lib::extractor::ExtractorConfig;
use douyin_monitor_lib::logging::Logger;
use douyin_monitor_lib::settings::Settings;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "需要真实浏览器，默认不参与常规测试"]
async fn cdp_end_to_end_extracts_works() -> anyhow::Result<()> {
    let Some(exe) = douyin_monitor_lib::cdp::find_browser("") else {
        eprintln!("跳过：未检测到 Chrome / Edge");
        return Ok(());
    };

    let work_dir = std::env::temp_dir().join(format!("douyin-monitor-test-{}", std::process::id()));
    std::fs::create_dir_all(&work_dir)?;
    let page_path: PathBuf = work_dir.join("fixture.html");
    std::fs::write(&page_path, FIXTURE_HTML)?;

    let logger = Logger::new();
    let settings = Settings {
        page_wait_secs: 2,
        scroll_rounds: 0,
        max_works: 20,
        ..Settings::default()
    };

    let profile = work_dir.join("profile");
    let browser = Browser::open(exe, profile, true, &logger).await?;
    let result = async {
        let url = format!("file:///{}", page_path.display().to_string().replace('\\', "/"));
        browser
            .goto(&url, Duration::from_secs(1), Duration::from_secs(30))
            .await?;
        let config = ExtractorConfig::from_settings(&settings);
        let parsed = collector::extract(&browser.page, &config, "").await?;
        Ok::<_, anyhow::Error>(parsed)
    }
    .await;
    browser.shutdown().await;

    let parsed = result?;
    assert_eq!(parsed.works.len(), 2, "应解析出 2 个作品");

    let first = parsed
        .works
        .iter()
        .find(|work| work.aweme_id == "7000000000000000001")
        .expect("应解析出第一个作品");
    assert_eq!(first.metrics.play, Some(12_000), "1.2万 应解析为 12000");
    assert_eq!(first.metrics.like, Some(3_456));
    assert_eq!(first.metrics.comment, Some(78));
    assert!(first.title.contains("第一条测试作品"));

    let second = parsed
        .works
        .iter()
        .find(|work| work.aweme_id == "7000000000000000002")
        .expect("应解析出第二个作品");
    assert_eq!(
        second.metrics.play,
        Some(340_000_000),
        "3.4亿 应解析为 340000000"
    );
    assert_eq!(second.metrics.share, Some(120));

    let _ = std::fs::remove_dir_all(&work_dir);
    Ok(())
}

const FIXTURE_HTML: &str = r#"<!doctype html>
<html lang="zh-CN">
  <head><meta charset="utf-8" /><title>作品数据 - 测试</title></head>
  <body>
    <div class="content-card">
      <a href="/video/7000000000000000001" title="第一条测试作品">
        <img alt="第一条测试作品 封面" src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" />
      </a>
      <div class="title">第一条测试作品</div>
      <div class="metrics">
        <span>播放量 1.2万</span>
        <span>点赞 3,456</span>
        <span>评论 78</span>
        <span>分享 12</span>
      </div>
      <div class="time">2 小时前</div>
    </div>
    <div class="content-card">
      <a href="/video/7000000000000000002" title="第二条测试作品">
        <img alt="第二条测试作品 封面" src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" />
      </a>
      <div class="title">第二条测试作品</div>
      <div class="metrics">
        <span>播放量 3.4亿</span>
        <span>点赞量 88.8万</span>
        <span>评论量 1234</span>
        <span>分享 120</span>
      </div>
      <div class="time">2026-09-20</div>
    </div>
  </body>
</html>
"#;
