# UpdateDialog 使用属性直传，不配 global Setting

base 中其余对话框（MessageDialog、ConfirmDialog 等）均通过 `global XXXSetting` 传递数据和显隐，但 `UpdateDialog` 有意选择基于 `Dialog` 容器、全部数据以 `in-out property` 直传、显隐由调用方控制。原因是 `Dialog` 本身就是属性驱动的组件，叠加 global 会形成两套传参机制；更新提示是单实例面板，global 的共享状态优势用不上。

_考虑过的方案_：自包含模式 + `global UpdatePanelSetting`（被拒，见上）。
