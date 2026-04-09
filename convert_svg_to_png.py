#!/usr/bin/env python3
"""
SVG to PNG converter for Tauri icon generation
使用这个脚本将SVG转换为PNG格式的图标

需要安装: pip install pillow cairosvg
使用方法: python convert_svg_to_png.py
"""

import os
import sys

def convert_svg_to_png():
    try:
        # 尝试导入所需的库
        try:
            import cairosvg
        except ImportError:
            print("❌ 请先安装 cairosvg: pip install cairosvg")
            return False
        
        # 文件路径
        svg_path = "src-tauri/icons/icon.svg"
        png_path = "src-tauri/icons/icon.png"
        
        # 检查SVG文件是否存在
        if not os.path.exists(svg_path):
            print(f"❌ SVG文件不存在: {svg_path}")
            return False
        
        print(f"🔄 正在转换 {svg_path} 到 {png_path}")
        print("📏 尺寸: 1024x1024 (推荐用于Tauri图标)")
        
        # 转换SVG到PNG
        cairosvg.svg2png(
            url=svg_path,
            write_to=png_path,
            output_width=1024,
            output_height=1024
        )
        
        print(f"✅ 转换成功! PNG图标已保存到: {png_path}")
        print(f"📝 现在可以运行: pnpm tauri icon {png_path}")
        return True
        
    except Exception as e:
        print(f"❌ 转换失败: {e}")
        return False

if __name__ == "__main__":
    print("🎨 Cursor Manager - SVG到PNG图标转换器")
    print("=" * 50)
    
    if convert_svg_to_png():
        print("\n🎉 转换完成! 接下来的步骤:")
        print("1. 运行: pnpm tauri icon ./src-tauri/icons/icon.png")
        print("2. 这将生成所有需要的应用图标尺寸")
    else:
        print("\n💡 其他转换方法:")
        print("1. 使用在线工具: https://cloudconvert.com/svg-to-png")
        print("2. 使用系统预览应用打开SVG后导出为PNG")
        print("3. 使用设计软件 (如Figma, Sketch等)")