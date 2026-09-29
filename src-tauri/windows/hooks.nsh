; ==============================================================================
; 抖音作品数据监控 (DouyinMonitor) - NSIS 自定义安装与卸载生命周期脚本
; ==============================================================================

!macro NSIS_HOOK_PREUNINSTALL
    ; 1. 强制终止可能仍在系统托盘后台运行的应用程序进程，解除文件占用防止卸载报错
    nsExec::Exec 'taskkill /F /IM DouyinMonitor.exe /T'
    nsExec::Exec 'taskkill /F /IM douyin-monitor.exe /T'

    ; 2. 清理开机自启动注册表项
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "DouyinMonitor"

    ; 3. 弹窗询问用户是否彻底清除本地数据与配置
    MessageBox MB_YESNO|MB_ICONQUESTION \
        "是否同时删除所有账号配置、监控数据与本地历史快照？$\r$\n$\r$\n【是】：彻底删除本地所有数据目录（含Cookie缓存与SQLite数据库）；$\r$\n【否】：仅卸载应用程序，保留数据以便未来重新安装。" \
        IDNO skip_cleanup

    ; 用户选择【是】：递归清理 AppData 下的持久化数据目录
    RMDir /r "$APPDATA\com.douyinmonitor.desktop"

skip_cleanup:
!macroend
