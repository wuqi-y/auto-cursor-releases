#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="staged_upload_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$OUT_DIR/files"

echo "[1/6] 检查是否有暂存内容..."
if git diff --cached --quiet; then
  echo "没有暂存变更，先执行 git add 后再运行本脚本。"
  exit 1
fi

echo "[2/6] 导出暂存文件清单..."
git diff --cached --name-only --diff-filter=ACMR > "$OUT_DIR/staged_files.txt"

echo "[3/6] 导出删除清单..."
git diff --cached --name-status --diff-filter=D > "$OUT_DIR/staged_deleted_files.txt" || true

echo "[4/6] 按清单复制文件到临时目录..."
while IFS= read -r f; do
  [ -z "$f" ] && continue
  mkdir -p "$OUT_DIR/files/$(dirname "$f")"
  cp -p "$f" "$OUT_DIR/files/$f"
done < "$OUT_DIR/staged_files.txt"

echo "[5/6] 生成网页提交说明..."
cat > "$OUT_DIR/README_UPLOAD.md" <<'EOF'
# GitHub 网页提交说明

1. 进入仓库网页 -> 目标分支
2. Add file -> Upload files
3. 把 `files/` 目录下的内容整体拖进去（保持目录结构）
4. 按 `staged_deleted_files.txt` 手动删除旧文件（若有）
5. 填写提交信息并 Commit changes

文件说明：
- `staged_files.txt`：本次上传文件列表
- `staged_deleted_files.txt`：需要手动删除的文件列表
EOF

echo "[6/6] 打包..."
tar -czf "$OUT_DIR.tar.gz" -C "$OUT_DIR" .

echo ""
echo "完成：$OUT_DIR.tar.gz"
echo "解压后按 $OUT_DIR/README_UPLOAD.md 操作即可。"
