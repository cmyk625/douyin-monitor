/// 登录状态探针：检测普通抖音网页版（www.douyin.com）的登录状态。
pub const LOGIN_PROBE_SCRIPT: &str = r#"(function () {
  var url = location.href;
  var text = (document.body && (document.body.innerText || document.body.textContent) || '').slice(0, 3000);
  var captchaEl = document.querySelector('#captcha_container, .secsdk_captcha_modal, [class*="captcha"]') !== null;
  var challenge = captchaEl || /请完成安全验证|拖动下方滑块|请进行验证|验证码校验|操作过于频繁|访问过于频繁/.test(text);

  var hasAvatar = document.querySelector('a[href*="/user/"], [data-e2e="user-info"], header img[class*="avatar"], .avatar-component') !== null;
  var hasUserMenu = /退出登录|个人中心|我的作品|我的喜欢|我的主页/.test(text);
  var cookieHasSession = /(?:^|;\s*)(?:sessionid|LOGIN_STATUS=1|login_status=1)/.test(document.cookie || '');

  var loginBtn = document.querySelector('[data-e2e="login-btn"], .login-btn, [class*="login-button"]');
  var loginPanel = document.querySelector('[data-e2e="login-panel"], .login-guide, [class*="login-mask"], [class*="login-dialog"]');
  var loginText = /扫码登录|验证码登录|手机号登录|立即登录|密码登录/.test(text);
  var loggedOut = /(?:login|passport|sso)\.douyin\.com/i.test(url) || (Boolean(loginBtn || loginPanel || loginText) && !hasAvatar && !hasUserMenu);

  var ready = document.readyState === 'complete';
  var loggedIn = ready && (hasAvatar || hasUserMenu || cookieHasSession) && !loggedOut && !challenge;

  return {
    url: url,
    title: document.title,
    loggedOut: loggedOut,
    challenge: challenge,
    loggedIn: loggedIn
  };
})()"#;

pub const SINGLE_VIDEO_SCRIPT: &str = r#"(async function () {
  const id = __ID__;
  const url = location.href;
  const text = (document.body && (document.body.innerText || document.body.textContent) || '').slice(0, 5000);

  const captchaEl = document.querySelector('#captcha_container, .secsdk_captcha_modal, [class*="captcha"]') !== null;
  const challenge = captchaEl || /请完成安全验证|拖动下方滑块|请进行验证|验证码校验|操作过于频繁|访问过于频繁/.test(text);

  // 严格比对：检查当前页面 URL 是否重定向到其他视频或推荐流
  const currentPathMatch = location.pathname.match(/\/(?:video|note)\/(\d+)/);
  const redirectedToOther = currentPathMatch && currentPathMatch[1] !== id;

  const isDeletedText = /该视频已被删除|视频不存在|作品已下架|内容已被删除|该作品已被删除/.test(text);
  const isPrivateText = /私密作品|私密视频|仅作者可见|创作者已将该作品设置为私密/.test(text);

  let video = null;
  let status = "not_found";
  let reason = "";

  if (challenge) {
    status = "challenge";
    reason = "页面要求安全验证（需人工处理滑块）";
  } else if (isDeletedText) {
    status = "deleted";
    reason = "视频已下架或删除";
  } else if (isPrivateText) {
    status = "private";
    reason = "视频为私密状态";
  } else if (redirectedToOther) {
    status = "redirected";
    reason = "未匹配到目标作品数据（页面已跳转推荐流）";
  }

  // 1. 尝试从当前页面同源上下文直接请求官方详情接口（最精准，含完整整数指标、发布时间、作者名）
  if (status !== "deleted" && status !== "private" && !challenge) {
    try {
      const resp = await fetch(
        '/aweme/v1/web/aweme/detail/?device_platform=webapp&aid=6383&channel=channel_pc_web&aweme_id=' +
          encodeURIComponent(id) +
          '&request_source=600&origin_type=video_page'
      );
      if (resp.ok) {
        const data = await resp.json();
        if (data && data.aweme_detail && String(data.aweme_detail.aweme_id) === id) {
          video = data.aweme_detail;
        }
      }
    } catch (_) {}
  }

  // 2. 备用：从页面全局状态和 JSON 脚本标签中深度遍历匹配
  if (!video && status !== "deleted" && status !== "private" && !challenge) {
    const seen = new Set();
    function find(value, depth) {
      if (!value || typeof value !== 'object' || seen.has(value) || depth > 30) return null;
      seen.add(value);
      if (String(value.aweme_id || value.awemeId || '') === id) return value;
      for (const item of Object.values(value)) {
        const hit = find(item, depth + 1);
        if (hit) return hit;
      }
      return null;
    }

    video = find(window._ROUTER_DATA, 0) || find(window.__INITIAL_STATE__, 0) || find(window._SSR_DATA, 0);
    if (!video) {
      for (const el of document.querySelectorAll('script[type="application/json"], script#RENDER_DATA, script#__UNIVERSAL_DATA_FOR_REHYDRATION__')) {
        try { video = find(JSON.parse(el.textContent), 0); } catch (_) {
          try { video = find(JSON.parse(decodeURIComponent(el.textContent)), 0); } catch (_) {}
        }
        if (video) break;
      }
    }
  }

  // 校验 video 对象中的删除或私密状态码
  if (video && video.status) {
    if (video.status.is_delete === true || video.status.status_code === 2004) {
      status = "deleted";
      reason = "视频已下架或删除";
      video = null;
    } else if (video.status.private_status === 1 || video.status.status_code === 2005) {
      status = "private";
      reason = "视频为私密状态";
      video = null;
    }
  }

  let works = [];
  if (video && status !== "deleted" && status !== "private" && !challenge) {
    const stats = video.statistics || {};
    const count = key => {
      const camel = key.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
      const value = stats[key] ?? stats[camel];
      return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : null;
    };

    // 公开页面通常无真实播放量，playCount 为 0 或缺失时设为 null
    let playVal = count('play_count');
    if (playVal === 0) {
      playVal = null;
    }

    const metrics = {
      play: playVal,
      like: count('digg_count'),
      comment: count('comment_count'),
      share: count('share_count'),
      collect: count('collect_count')
    };

    const authorName = String(video.author?.nickname || video.author?.name || '').trim();
    const title = String(video.desc || video.title || '').trim();
    const publishedAt = typeof video.create_time === 'number' && video.create_time > 0 ? video.create_time : null;

    const available = Object.values(metrics).some(v => v !== null);
    if (available) {
      status = "ok";
      works.push({
        aweme_id: id,
        title: title,
        author_name: authorName,
        url: 'https://www.douyin.com/video/' + id,
        published_at: publishedAt,
        metrics: metrics,
        is_approximate: false
      });
    } else {
      if (status === "not_found") {
        reason = "该视频页面未提供可读取的公开指标";
      }
    }
  }

  // 3. 兜底：如果 API 和全局变量均未取到，从已渲染的 DOM 元素中提取
  if (works.length === 0 && status !== "deleted" && status !== "private" && !challenge && !redirectedToOther) {
    const parseNum = str => {
      if (!str) return null;
      str = String(str).replace(/[\s,，]/g, '').trim();
      const m = str.match(/([\d.]+)\s*([万wW亿]?)/);
      if (!m) return null;
      let val = parseFloat(m[1]);
      if (isNaN(val)) return null;
      if (m[2] === '万' || m[2] === 'w' || m[2] === 'W') val = Math.round(val * 10000);
      else if (m[2] === '亿') val = Math.round(val * 100000000);
      return val;
    };

    const getElText = selector => {
      const el = document.querySelector(selector);
      return el ? (el.textContent || el.innerText || '').trim() : '';
    };

    const likeText = getElText('[data-e2e="video-player-digg"] [data-e2e="like-count"], [data-e2e="like-count"], [data-e2e="video-player-digg"]');
    const commentText = getElText('[data-e2e="feed-comment-icon"] [data-e2e="comment-count"], [data-e2e="comment-count"], [data-e2e="feed-comment-icon"]');
    const collectText = getElText('[data-e2e="video-player-collect"] [data-e2e="collect-count"], [data-e2e="collect-count"], [data-e2e="video-player-collect"]');
    const shareText = getElText('[data-e2e="video-player-share"] [data-e2e="video-share-icon-container"], [data-e2e="video-share-icon-container"], [data-e2e="video-player-share"]');

    const domMetrics = {
      play: null,
      like: parseNum(likeText),
      comment: parseNum(commentText),
      share: parseNum(shareText),
      collect: parseNum(collectText)
    };

    const isApprox = /[万wW亿]/.test(likeText + commentText + collectText + shareText);

    // 作者名称
    let authorName = '';
    const authorEl = document.querySelector('[data-e2e="user-info"] a[href*="/user/"], [data-e2e="user-info"] [class*="name"]');
    if (authorEl) {
      authorName = (authorEl.textContent || authorEl.innerText || '').trim();
    }
    if (!authorName) {
      const avatarImg = document.querySelector('[data-e2e="user-info"] img[alt]');
      if (avatarImg && avatarImg.alt) {
        authorName = avatarImg.alt.replace(/的头像$/, '').trim();
      }
    }

    // 标题
    const descEl = document.querySelector('[data-e2e="video-desc"], [class*="video-info-detail"] [class*="title"], h1');
    let title = descEl ? (descEl.textContent || descEl.innerText || '').trim() : '';
    if (!title) {
      title = document.title.replace(/\s*-\s*抖音.*$/, '').trim();
    }

    // 发布时间解析
    let publishedAt = null;
    const timeEl = document.querySelector('[data-e2e="detail-video-publish-time"], [class*="publish-time"]');
    if (timeEl) {
      const timeMatch = (timeEl.textContent || '').match(/(\d{4}[-/年]\d{1,2}[-/月]\d{1,2}(?:\s+\d{1,2}:\d{2}(?::\d{2})?)?)/);
      if (timeMatch) {
        const normalized = timeMatch[1].replace(/年|月/g, '-').replace(/日/g, '');
        const ts = Date.parse(normalized);
        if (!isNaN(ts)) {
          publishedAt = Math.floor(ts / 1000);
        }
      }
    }

    const domAvailable = Object.values(domMetrics).some(v => v !== null);
    if (domAvailable) {
      status = "ok";
      works.push({
        aweme_id: id,
        title: title,
        author_name: authorName,
        url: 'https://www.douyin.com/video/' + id,
        published_at: publishedAt,
        metrics: domMetrics,
        is_approximate: isApprox
      });
    }
  }

  if (works.length === 0 && reason === "") {
    reason = redirectedToOther
      ? "未匹配到目标作品数据（页面已跳转推荐流）"
      : "该视频页面未解析到有效数据，请确认链接可正常打开";
  }

  return {
    ok: works.length > 0,
    status: status,
    blocked: status === "challenge",
    reason: reason,
    pageUrl: url,
    pageTitle: document.title,
    works: works,
    debug: {
      target: id,
      videoFound: Boolean(video),
      redirectedToOther: redirectedToOther
    }
  };
})()"#;

/// 在页面上下文执行的解析脚本。__CFG__ 会替换为 JSON 配置。
/// 返回结构：{ ok, blocked, reason, pageUrl, pageTitle, works: [...], debug: {...} }
pub const SCRIPT_TEMPLATE: &str = r#"(function () {
  var cfg = __CFG__;
  var esc = function (s) { return String(s).replace(/[.*+?^${}()|[\]\\]/g, '\\$&'); };
  var toNum = function (text) {
    if (text === null || text === undefined) return null;
    var m = String(text).replace(/[\s,，]/g, '').match(/(\d+(?:\.\d+)?)(亿|万|[wW])?/);
    if (!m) return null;
    var n = parseFloat(m[1]);
    if (!isFinite(n) || n < 0) return null;
    var f = m[2] === '亿' ? 100000000 : (m[2] === '万' || m[2] === 'w' || m[2] === 'W') ? 10000 : 1;
    return Math.round(n * f);
  };
  var textOf = function (el) { return el ? String(el.innerText || el.textContent || '') : ''; };
  var metricFrom = function (text, labels) {
    for (var i = 0; i < labels.length; i++) {
      var l = esc(labels[i]);
      var m = text.match(new RegExp(l + '\\s*[:：]?\\s*([0-9][0-9,]*(?:\\.[0-9]+)?\\s*(?:亿|万|[wW])?)'));
      if (m) { var v = toNum(m[1]); if (v !== null) return v; }
      m = text.match(new RegExp('([0-9][0-9,]*(?:\\.[0-9]+)?\\s*(?:亿|万|[wW])?)\\s*' + l));
      if (m) { var v2 = toNum(m[1]); if (v2 !== null) return v2; }
    }
    return null;
  };
  var parseTime = function (text) {
    var now = Math.round(Date.now() / 1000);
    var m = text.match(/(\d+)\s*秒前/); if (m) return now - parseInt(m[1], 10);
    m = text.match(/(\d+)\s*分钟前/); if (m) return now - parseInt(m[1], 10) * 60;
    m = text.match(/(\d+)\s*小时前/); if (m) return now - parseInt(m[1], 10) * 3600;
    m = text.match(/(\d+)\s*天前/); if (m) return now - parseInt(m[1], 10) * 86400;
    if (/刚刚/.test(text)) return now;
    if (/昨天/.test(text)) return now - 86400;
    m = text.match(/(20\d{2})\s*[-/年]\s*(\d{1,2})\s*[-/月]\s*(\d{1,2})/);
    if (m) return Math.round(new Date(+m[1], +m[2] - 1, +m[3]).getTime() / 1000);
    m = text.match(/(\d{1,2})\s*[-/月]\s*(\d{1,2})\s*日?/);
    if (m) {
      var d = new Date(new Date().getFullYear(), +m[1] - 1, +m[2]);
      if (d.getTime() > Date.now() + 86400000) d.setFullYear(d.getFullYear() - 1);
      return Math.round(d.getTime() / 1000);
    }
    return null;
  };
  var idFrom = function (url) { var m = String(url).match(/\/(?:video|note)\/(\d+)/); return m ? m[1] : ''; };
  var labels = Object.assign({
    play: ['播放量', '播放'],
    like: ['点赞量', '点赞'],
    comment: ['评论量', '评论'],
    share: ['分享量', '分享', '转发量', '转发'],
    collect: ['收藏量', '收藏']
  }, cfg.labels || {});
  var bodyText = textOf(document.body).slice(0, 3000);
  var url = location.href;
  var loggedOut = /(login|passport)/i.test(url) || /扫码登录|验证码登录|手机号登录|登录后查看|立即登录|请先登录|密码登录/.test(bodyText);
  var challenge = /请完成安全验证|拖动下方滑块|请进行验证|验证码校验|操作过于频繁|访问过于频繁/.test(bodyText);

  var picked = [];
  var seen = {};
  var push = function (cardEl, anchorEl) {
    var cardText = textOf(cardEl).slice(0, 1500);
    var href = anchorEl ? String(anchorEl.getAttribute('href') || '') : '';
    var absUrl = href ? (href.indexOf('http') === 0 ? href : location.origin + href) : '';
    var awemeId = idFrom(absUrl);
    var metrics = {};
    var any = false;
    var keys = Object.keys(labels);
    for (var i = 0; i < keys.length; i++) {
      var value = metricFrom(cardText, labels[keys[i]]);
      metrics[keys[i]] = value;
      if (value !== null) any = true;
    }
    if (!any && !awemeId) return;
    var key = awemeId || absUrl || cardText.slice(0, 80);
    if (seen[key]) return;
    seen[key] = true;
    var title = '';
    var img = cardEl.querySelector ? cardEl.querySelector('img[alt]') : null;
    if (img) title = String(img.getAttribute('alt') || '').trim();
    if (!title && anchorEl) title = String(anchorEl.getAttribute('title') || '').trim();
    if (!title) {
      var lines = cardText.split('\n').map(function (s) { return s.trim(); })
        .filter(function (s) { return s && !/^(播放|点赞|评论|分享|收藏|转发)/.test(s); });
      title = lines[0] || '';
    }
    picked.push({
      aweme_id: awemeId || key.slice(0, 48),
      title: title.slice(0, 200),
      url: absUrl,
      published_at: parseTime(cardText),
      metrics: metrics
    });
  };

  var source = 'none';
  var selectors = cfg.cardSelectors || [];
  for (var s = 0; s < selectors.length; s++) {
    var cards = Array.prototype.slice.call(document.querySelectorAll(selectors[s]));
    if (!cards.length) continue;
    source = 'selector';
    for (var c = 0; c < cards.length; c++) {
      push(cards[c], cards[c].querySelector('a[href*="/video/"], a[href*="/note/"]'));
    }
    if (picked.length >= (cfg.maxWorks || 50)) break;
  }
  if (!picked.length) {
    var anchors = Array.prototype.slice.call(document.querySelectorAll('a[href*="/video/"], a[href*="/note/"]'));
    if (anchors.length) source = 'anchor';
    for (var a = 0; a < anchors.length; a++) {
      var card = anchors[a].closest('[class*="card"], [class*="item"], li, tr') || anchors[a].parentElement;
      if (card && textOf(card).replace(/\s/g, '').length < 8) card = card.parentElement || card;
      push(card || anchors[a], anchors[a]);
    }
  }
  var works = picked.slice(0, cfg.maxWorks || 50);
  return {
    ok: works.length > 0,
    blocked: loggedOut || challenge,
    reason: challenge ? '页面要求安全验证（需人工处理）' : (loggedOut ? '需要登录创作者中心' : ''),
    pageUrl: url,
    pageTitle: document.title,
    works: works,
    debug: {
      source: source,
      parsed: works.length,
      anchors: document.querySelectorAll('a[href*="/video/"], a[href*="/note/"]').length,
      bodySample: bodyText.slice(0, 400)
    }
  };
})()"#;
