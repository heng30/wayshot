# Wayshot UI

Wayshot 的 Slint UI 组件库。base 目录提供可复用组件，panel 等上层目录组合它们构成完整界面。

## Language

**更新提示对话框 (UpdateDialog)**:
提示用户软件有新版本可用的对话框，基于 Dialog 容器。
_Avoid_: 更新面板、升级弹窗

**不再提示 (dont-ask-again)**:
用户选择后不再展示更新提示的状态。勾选即生效，与之后点击哪个按钮无关；由调用方持久化，组件不保存。
_Avoid_: 跳过提醒、记住选择

**更新内容 (update-content)**:
本次版本更新的 changelog 文本，在折叠区（SingleCollapse）中展开查看。为空时折叠区整体隐藏。
_Avoid_: 更新日志、release notes

**版本行 (version line)**:
正文中"当前版本 vX ｜ 最新版本 vY"的次要色小字行，与提示语分离展示。
