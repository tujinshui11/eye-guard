# 生成护眼助手图标 assets/icon.png（256x256）——深海蓝主题
#   深蓝圆底（#0B0F1A → #131A2E）+ 品牌蓝 #4D6BFE / 青色 #38BDF8 抽象鲸尾双弧 + 气泡点
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
$g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality

# 透明底（外圈保留 4px 透明边）
$g.Clear([System.Drawing.Color]::Transparent)

# 深蓝圆底：对角渐变 #131A2E → #0B0F1A
$rect = New-Object System.Drawing.Rectangle(4, 4, 248, 248)
$cTop = [System.Drawing.Color]::FromArgb(255, 0x13, 0x1A, 0x2E)
$cBot = [System.Drawing.Color]::FromArgb(255, 0x0B, 0x0F, 0x1A)
$bgBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rect, $cTop, $cBot, 60.0)
$g.FillEllipse($bgBrush, $rect)
$bgBrush.Dispose()

# 深海光晕外环（青色 #38BDF8，低透明度）
$glow = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(60, 0x38, 0xBD, 0xF8), 4)
$g.DrawEllipse($glow, 12, 12, 232, 232)
$glow.Dispose()

$brand = [System.Drawing.Color]::FromArgb(255, 0x4D, 0x6B, 0xFE)
$cyan = [System.Drawing.Color]::FromArgb(255, 0x38, 0xBD, 0xF8)

# 鲸尾躯干（青色竖干，自底部向上）
$stem = New-Object System.Drawing.Drawing2D.GraphicsPath
$stem.StartFigure()
$stem.AddLine(128.0, 198.0, 128.0, 168.0)
$penStem = New-Object System.Drawing.Pen($cyan, 15)
$penStem.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
$penStem.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
$g.DrawPath($penStem, $stem)
$penStem.Dispose()

# 左尾鳍（品牌蓝，向上左扬）
$leftFin = New-Object System.Drawing.Drawing2D.GraphicsPath
$leftFin.StartFigure()
$leftFin.AddBezier(128.0, 168.0, 108.0, 148.0, 86.0, 122.0, 76.0, 92.0)
$penL = New-Object System.Drawing.Pen($brand, 17)
$penL.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
$penL.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
$g.DrawPath($penL, $leftFin)
$penL.Dispose()

# 右尾鳍（青色，向上右扬；与躯干交汇成 V 形尾鳍）
$rightFin = New-Object System.Drawing.Drawing2D.GraphicsPath
$rightFin.StartFigure()
$rightFin.AddBezier(128.0, 168.0, 148.0, 148.0, 170.0, 122.0, 180.0, 92.0)
$penR = New-Object System.Drawing.Pen($cyan, 17)
$penR.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
$penR.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
$g.DrawPath($penR, $rightFin)
$penR.Dispose()

# 气泡点（浅蓝白，右上）
$bubble = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 0xDC, 0xEB, 0xFF))
$g.FillEllipse($bubble, 166, 60, 18, 18)
$bubble.Dispose()

# 小气泡（更小的辅助点）
$bubble2 = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(200, 0xBF, 0xE3, 0xFF))
$g.FillEllipse($bubble2, 190, 92, 10, 10)
$bubble2.Dispose()

$g.Dispose()

$outPath = Join-Path $outDir "icon.png"
$bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output "icon written: $outPath"
