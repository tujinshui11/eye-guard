# 生成护眼助手图标 assets/icon.png（256x256）——深色底 + 暖橙光环
# 用法：powershell -NoProfile -ExecutionPolicy Bypass -File scripts/make-icon.ps1
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$outDir = Join-Path $root "assets"
if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir | Out-Null }

$size = 256
$bmp = New-Object System.Drawing.Bitmap($size, $size)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias

# 透明底
$g.Clear([System.Drawing.Color]::Transparent)

# 深色圆底（#1C2026）
$bgBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 28, 32, 38))
$g.FillEllipse($bgBrush, 4, 4, 248, 248)

# 外圈暖橙光环（#FF9E42）
$pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(255, 255, 158, 66), 22)
$g.DrawEllipse($pen, 52, 52, 152, 152)

# 中心暖橙圆点
$dot = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 255, 158, 66))
$g.FillEllipse($dot, 106, 106, 44, 44)

$g.Dispose()

$outPath = Join-Path $outDir "icon.png"
$bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output "icon written: $outPath"
