# Cursor自动注册 - 可执行文件

## 📦 打包信息
- 平台: macos
- 可执行文件: cursor_register, cdp_flow_runner
- 打包时间: 2026-04-22 11:08:01

## 🚀 使用方法

```bash
# 1) Cursor 注册（默认启用无痕模式和银行卡绑定）
./cursor_register test@example.com John Smith

# 只提供邮箱（会生成随机姓名）
./cursor_register test@example.com

# 完整参数用法
./cursor_register test@example.com John Smith true . true 0 '{"btnIndex":1}'

# 启用跳过手机号验证（实验性功能）
./cursor_register test@example.com John Smith true . true 1 '{"btnIndex":1}'

# 注册美国账户（使用按钮索引2）
./cursor_register test@example.com John Smith true . true 0 '{"btnIndex":2}'

# 参数说明:
# 参数1: 邮箱地址 (必需)
# 参数2: 名字 (可选，默认: Auto)
# 参数3: 姓氏 (可选，默认: Generated)
# 参数4: 无痕模式 (可选，默认: true)
# 参数5: 应用目录 (可选，默认: .)
# 参数6: 银行卡绑定 (可选，默认: true)
# 参数7: 跳过手机号验证 (可选，默认: 0，设置为1启用实验性功能)
# 参数8: 配置JSON (可选，默认: {})
#        - btnIndex: 按钮索引，1=默认地区，2=美国账户

# 2) CDP 流程运行器（与你在 python 里调用 cdp_flow_runner.py 参数一致）
./cdp_flow_runner --url "https://chatgpt.com/" --click "css:button[class*='btn-secondary']"
```

## 📊 响应格式

成功:
```json
{"success": true, "email": "test@example.com", "message": "注册成功"}
```

失败:
```json
{"success": false, "error": "错误信息"}
```

## ⚠️ 注意事项

1. 需要Chrome/Chromium浏览器
2. 需要稳定的网络连接
3. 首次运行可能需要较长时间加载
