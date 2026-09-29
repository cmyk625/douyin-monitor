/** @type {import('cz-git').UserConfig} */
module.exports = {
  extends: ["@commitlint/config-conventional"],
  rules: {
    // 提交类型枚举
    "type-enum": [
      2,
      "always",
      [
        "feat",     // 新功能
        "fix",      // 修复 bug
        "docs",     // 文档更新
        "style",    // 代码格式变动（不影响代码运行逻辑）
        "refactor", // 重构代码
        "perf",     // 性能优化
        "test",     // 测试用例
        "build",    // 打包/构建系统变动
        "ci",       // 持续集成相关变动
        "chore",    // 其他杂项/构建工具变更
        "revert",   // 代码回滚
      ],
    ],
    // 主题大小写不限制（支持中文及驼峰/小写英文）
    "subject-case": [0],
    // 主题内容不能为空
    "subject-empty": [2, "never"],
    // 类型不能为空
    "type-empty": [2, "never"],
  },
  prompt: {
    messages: {
      type: "选择你要提交的类型 :",
      scope: "选择一个提交范围（可选）:",
      customScope: "请输入自定义的提交范围 :",
      subject: "填写简短精炼的变更描述 :\n",
      body: '填写更加详细的变更描述（可选）。使用 "|" 换行 :\n',
      breaking: '列举非兼容性重大变更（可选）。使用 "|" 换行 :\n',
      footerPrefixesSelect: "选择关联 issue 前缀（可选）:",
      customFooterPrefix: "输入自定义 issue 前缀 :",
      footer: "列举关联 issue（可选）例如: #31, #I3244 :\n",
      confirmCommit: "是否确认以以上格式生成 commit ?",
    },
    types: [
      { value: "feat", name: "feat:     ✨  新增功能 | A new feature" },
      { value: "fix", name: "fix:      🐛  修复缺陷 | A bug fix" },
      { value: "docs", name: "docs:     📝  文档更新 | Documentation only changes" },
      { value: "style", name: "style:    💄  代码格式 | Changes that do not affect the meaning of the code" },
      { value: "refactor", name: "refactor: ♻️   代码重构 | A code change that neither fixes a bug nor adds a feature" },
      { value: "perf", name: "perf:     ⚡️  性能提升 | A code change that improves performance" },
      { value: "test", name: "test:     ✅  测试相关 | Adding missing tests or correcting existing tests" },
      { value: "build", name: "build:    📦️  构建打包 | Changes that affect the build system or external dependencies" },
      { value: "ci", name: "ci:       🎡  持续集成 | Changes to our CI configuration files and scripts" },
      { value: "chore", name: "chore:    🔨  其他修改 | Other changes that don't modify src or test files" },
      { value: "revert", name: "revert:   ⏪️  回退代码 | Revert to a commit" },
    ],
    useEmoji: false,
    scopes: [
      { value: "works", name: "works:       监控作品模块" },
      { value: "accounts", name: "accounts:    采集账号管理" },
      { value: "rules", name: "rules:       报警规则与计算" },
      { value: "feishu", name: "feishu:      飞书通知与配置" },
      { value: "dashboard", name: "dashboard:   仪表盘与概览" },
      { value: "alerts", name: "alerts:      报警历史明细" },
      { value: "settings", name: "settings:    系统偏好设置" },
      { value: "installer", name: "installer:   安装包与升级脚本" },
      { value: "ui", name: "ui:          通用 UI 组件" },
      { value: "deps", name: "deps:        项目依赖更新" },
    ],
    allowCustomScopes: true,
    allowEmptyScopes: true,
    customScopesAlias: "custom:   自定义范围",
    emptyScopesAlias: "empty:    跳过范围",
    skipQuestions: ["body", "breaking", "footerPrefixesSelect", "customFooterPrefix", "footer"],
  },
};
