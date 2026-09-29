; ==============================================================================
; 抖音作品数据监控 (DouyinMonitor) - NSIS 自定义安装与卸载生命周期脚本
; ==============================================================================

!macro NSIS_HOOK_PREINSTALL
    ; 1. 安装/覆盖安装/重新安装前：强制关闭正在运行的应用程序（解除托盘驻留进程文件占用，防止覆盖报错）
    nsExec::Exec 'taskkill /F /IM DouyinMonitor.exe /T'
    nsExec::Exec 'taskkill /F /IM douyin-monitor.exe /T'
    Sleep 600
!macroend

!macro NSIS_HOOK_PREUNINSTALL
    ; 1. 卸载或更新旧版本时：强制终止正在运行的应用程序进程，解除文件占用防止卸载报错
    nsExec::Exec 'taskkill /F /IM DouyinMonitor.exe /T'
    nsExec::Exec 'taskkill /F /IM douyin-monitor.exe /T'
    Sleep 600

    ; 2. 清理开机自启动注册表项
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "DouyinMonitor"

    ; 注意：绝不删除 %APPDATA%\com.douyinmonitor.desktop 中的本地数据库与用户配置！
    ; 确保无论用户是重新安装、覆盖安装、新版本升级，甚至卸载后再次安装，
    ; 所有的监控视频、历史快照、登录 Cookie 会话、飞书通知与报警规则等核心数据均永久保留、绝不丢失！
!macroend
