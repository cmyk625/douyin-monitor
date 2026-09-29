const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync('src-tauri/src/extractor_js.rs', 'utf8');
const script = name => source.match(new RegExp(`pub const ${name}: &str = r#"([\\s\\S]*?)"#;`))[1];
const probe = (text, readyState = 'complete') => vm.runInNewContext(script('LOGIN_PROBE_SCRIPT'), {
  location: { href: 'https://creator.douyin.com/creator-micro/home' },
  document: { title: '', readyState, body: text === null ? null : { innerText: text } },
});
assert.equal(probe(null).loggedIn, false);
assert.equal(probe('加载中').loggedIn, false);
assert.equal(probe('作品管理 数据中心', 'loading').loggedIn, false);
assert.equal(probe('作品管理 数据中心 扫码登录').loggedIn, false);
assert.equal(probe('作品管理 数据中心 请完成安全验证').loggedIn, false);
assert.equal(probe('作品管理 数据中心').loggedIn, true);
const videos = Array.from({ length: 60 }, (_, i) => ({ aweme_id: String(i), statistics: { digg_count: i } }));
function extract(data) {
  return vm.runInNewContext(script('SINGLE_VIDEO_SCRIPT').replace('__ID__', '"50"'), {
    window: { _ROUTER_DATA: data },
    location: { href: 'https://www.douyin.com/video/50' },
    document: { title: '', body: { innerText: '' }, querySelectorAll: () => [] },
  });
}
const result = extract(videos);
assert.equal(result.works.length, 1);
assert.equal(result.works[0].aweme_id, '50');
assert.equal(result.works[0].metrics.like, 50);
assert.equal(result.works[0].metrics.play, null);
assert.equal(extract(videos.slice(0, 50)).works.length, 0);
assert.equal(extract(null).works.length, 0);
console.log('登录探针与单视频范围回归检查通过');
