use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use serde::{Deserialize, Serialize};

static CUSTOM_CONFIG_PATH: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub feishu: FeishuConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub webhook: String,
    #[serde(default)]
    pub secret: String,
    #[serde(default)]
    pub keyword: String,
}

fn default_true() -> bool {
    true
}

impl Default for FeishuConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            webhook: String::new(),
            secret: String::new(),
            keyword: String::new(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            feishu: FeishuConfig::default(),
        }
    }
}

impl AppConfig {
    /// 注册应用数据目录作为备选配置路径（避免生产安装于 Program Files 等无写权限目录时无法创建）
    pub fn init_base_dir(dir: &Path) {
        let candidate = dir.join("config.toml");
        let _ = CUSTOM_CONFIG_PATH.set(candidate);
    }

    /// 寻找或确定 config.toml 的有效路径
    pub fn config_path() -> PathBuf {
        // 1. 若应用数据目录下的 config.toml 已存在，优先使用
        if let Some(explicit) = CUSTOM_CONFIG_PATH.get() {
            if explicit.exists() {
                return explicit.clone();
            }
        }

        // 2. 优先检查当前工作目录（开发环境项目根目录）
        if let Ok(cwd) = std::env::current_dir() {
            let p = cwd.join("config.toml");
            if p.exists() {
                return p;
            }
        }

        // 3. 检查可执行文件所在目录（便携运行 / 生产安装包同级已有配置）
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let p = dir.join("config.toml");
                if p.exists() {
                    return p;
                }
            }
        }

        // 4. 首次未创建时：若当前工作目录下有 Cargo.toml，优先在项目根目录生成（开发调试便利）
        if let Ok(cwd) = std::env::current_dir() {
            if cwd.join("Cargo.toml").exists() || cwd.join("package.json").exists() {
                return cwd.join("config.toml");
            }
        }

        // 5. 若已注册应用数据目录，在数据目录下生成（有可靠写入权限）
        if let Some(explicit) = CUSTOM_CONFIG_PATH.get() {
            return explicit.clone();
        }

        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                return dir.join("config.toml");
            }
        }

        PathBuf::from("config.toml")
    }

    /// 加载配置；若文件尚不存在，则自动初始化
    pub fn load_or_init(initial_feishu: Option<FeishuConfig>) -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<AppConfig>(&content) {
                    return cfg;
                }
            }
        }

        let config = AppConfig {
            feishu: initial_feishu.unwrap_or_default(),
        };
        let _ = config.save_to_path(&path);
        config
    }

    /// 即时读取最新配置（支持动态热重载，修改后无需重启）
    pub fn load() -> Self {
        Self::load_or_init(None)
    }

    /// 将配置格式化为带有详细中文注释的 TOML 并写入文件
    pub fn save_to_path(&self, path: &PathBuf) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let content = format!(
            r#"# 抖音监控系统配置文件 (config.toml)

[feishu]
# 是否启用飞书群机器人报警推送（true 为启用，false 为暂停）
enabled = {enabled}

# 飞书群自定义机器人的 Webhook 地址
# 示例："https://open.feishu.cn/open-apis/bot/v2/hook/xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
webhook = "{webhook}"

# 飞书自定义机器人安全设置：签名校验密钥（如群机器人安全设置中开启了“签名校验”请在此填写；未开启留空即可）
secret = "{secret}"

# 飞书自定义机器人安全设置：自定义关键词（如群机器人安全设置中开启了“关键词验证”请在此填写；报警内容将自动带上此关键词）
keyword = "{keyword}"
"#,
            enabled = self.feishu.enabled,
            webhook = self.feishu.webhook,
            secret = self.feishu.secret,
            keyword = self.feishu.keyword,
        );

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(path, content)
    }
}
