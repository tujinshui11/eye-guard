# Eye-guard app icon generator -- C2 design (squircle base + minimal V mark)
# Outputs: assets/icon.png (256px master) + assets/icon-sizes/{16,24,32,48,64,128,256}.png
# Multi-size PNGs are packed into multi-entry ICO by scripts/make-ico.js
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File scripts/make-icon.ps1
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$assets = Join-Path $root "assets"
$sizesDir = Join-Path $assets "icon-sizes"
if (-not (Test-Path $sizesDir)) { New-Item -ItemType Directory -Path $sizesDir -Force | Out-Null }

function New-SquirclePath([double]$x, [double]$y, [double]$w, [double]$h, [double]$r) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = $r * 2
  $p.AddArc([float]$x, [float]$y, [float]$d, [float]$d, 180, 90)
  $p.AddArc([float]($x + $w - $d), [float]$y, [float]$d, [float]$d, 270, 90)
  $p.AddArc([float]($x + $w - $d), [float]($y + $h - $d), [float]$d, [float]$d, 0, 90)
  $p.AddArc([float]$x, [float]($y + $h - $d), [float]$d, [float]$d, 90, 90)
  $p.CloseFigure()
  return $p
}

function Render-Icon([int]$size) {
  $bmp = New-Object System.Drawing.Bitmap($size, $size)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.Clear([System.Drawing.Color]::Transparent)

  $s = $size / 256.0

  # squircle base (#1E2233 -> #0A0E1A subtle diagonal gradient)
  $base = New-SquirclePath (8*$s) (8*$s) (240*$s) (240*$s) (56*$s)
  $grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Rectangle([int](8*$s), [int](8*$s), [int](240*$s), [int](240*$s))),
    [System.Drawing.Color]::FromArgb(255, 0x1E, 0x22, 0x33),
    [System.Drawing.Color]::FromArgb(255, 0x0A, 0x0E, 0x1A),
    60.0)
  $g.FillPath($grad, $base)
  $grad.Dispose(); $base.Dispose()

  $blue = [System.Drawing.Color]::FromArgb(255, 0x0A, 0x84, 0xFF)
  $cyan = [System.Drawing.Color]::FromArgb(255, 0x64, 0xD2, 0xFF)

  # minimal V mark (two bezier strokes, blue -> cyan gradient, round caps)
  $v = New-Object System.Drawing.Drawing2D.GraphicsPath
  $v.AddBezier([float](76*$s), [float](84*$s), [float](96*$s), [float](140*$s), [float](116*$s), [float](164*$s), [float](128*$s), [float](172*$s))
  $v.AddBezier([float](128*$s), [float](172*$s), [float](140*$s), [float](164*$s), [float](160*$s), [float](140*$s), [float](180*$s), [float](84*$s))
  $vBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Rectangle([int](60*$s), [int](76*$s), [int](140*$s), [int](104*$s))), $blue, $cyan, 0.0)
  $penV = New-Object System.Drawing.Pen($vBrush, [float](24*$s))
  $penV.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
  $penV.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
  $penV.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
  $g.DrawPath($penV, $v)
  $penV.Dispose(); $vBrush.Dispose(); $v.Dispose()

  $g.Dispose()
  return $bmp
}

foreach ($sz in @(16, 24, 32, 48, 64, 128, 256)) {
  $b = Render-Icon $sz
  $outPath = Join-Path $sizesDir ("{0}.png" -f $sz)
  $b.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
  $b.Dispose()
  Write-Output ("icon-size written: {0}" -f $outPath)
}

# master (256px) also lands at assets/icon.png (kept for compatibility)
Copy-Item (Join-Path $sizesDir "256.png") (Join-Path $assets "icon.png") -Force
Write-Output ("icon master written: {0}" -f (Join-Path $assets "icon.png"))
