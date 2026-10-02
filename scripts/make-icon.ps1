# Eye-guard (Ace) app icon generator -- K4 design
# Cream graph-paper squircle base + single warm arc (soft shadow)
# Outputs: assets/icon.png (256px master) + assets/icon-sizes/{16,24,32,48,64,128,256}.png
# Multi-size PNGs are packed into multi-entry ICO by scripts/make-ico.js
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File scripts/make-icon.ps1
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$assets = Join-Path $root "assets"
$sizesDir = Join-Path $assets "icon-sizes"
if (-not (Test-Path $sizesDir)) { New-Item -ItemType Directory -Path $sizesDir -Force | Out-Null }

function New-Squircle([double]$x, [double]$y, [double]$w, [double]$h, [double]$r) {
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

  # cream paper squircle base (#F8F0E6 -> #F1E3D1)
  $base = New-Squircle (10*$s) (10*$s) (236*$s) (236*$s) (66*$s)
  $bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Rectangle(0, 0, $size, $size)),
    [System.Drawing.Color]::FromArgb(255, 0xF8, 0xF0, 0xE6),
    [System.Drawing.Color]::FromArgb(255, 0xF1, 0xE3, 0xD1), 60.0)
  $g.FillPath($bg, $base)
  $bg.Dispose()

  # graph paper grid (clipped inside squircle; thicker relative weight at small sizes)
  $g.SetClip($base)
  $gridAlpha = 60
  if ($size -le 24) { $gridAlpha = 90 }
  $gridPen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb($gridAlpha, 0xB9, 0x9A, 0x7A), [float](1.6*$s))
  for ($i = 8; $i -lt 256; $i += 24) {
    $g.DrawLine($gridPen, [float]($i*$s), 0.0, [float]($i*$s), [float]$size)
    $g.DrawLine($gridPen, 0.0, [float]($i*$s), [float]$size, [float]($i*$s))
  }
  $gridPen.Dispose()
  $g.ResetClip()
  $base.Dispose()

  # single warm arc with soft shadow
  $penW = 26.0
  if ($size -le 24) { $penW = 34.0 }  # small sizes: bolder stroke for legibility
  $sh = New-Object System.Drawing.Drawing2D.GraphicsPath
  $sh.AddBezier([float](66*$s), [float](146*$s), [float](96*$s), [float](96*$s), [float](160*$s), [float](96*$s), [float](190*$s), [float](146*$s))
  $shadowPen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(55, 0x3A, 0x1E, 0x0E), [float]((($penW+1)*$s)))
  $shadowPen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
  $shadowPen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
  $g.DrawPath($shadowPen, $sh)
  $shadowPen.Dispose(); $sh.Dispose()

  $arc = New-Object System.Drawing.Drawing2D.GraphicsPath
  $arc.AddBezier([float](66*$s), [float](142*$s), [float](96*$s), [float](92*$s), [float](160*$s), [float](92*$s), [float](190*$s), [float](142*$s))
  $ab = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Rectangle([int](60*$s), [int](90*$s), [int](140*$s), [int](60*$s))),
    [System.Drawing.Color]::FromArgb(255, 0xFF, 0xAE, 0x6E),
    [System.Drawing.Color]::FromArgb(255, 0xFF, 0x7E, 0x50), 0.0)
  $pen = New-Object System.Drawing.Pen($ab, [float]($penW*$s))
  $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
  $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
  $g.DrawPath($pen, $arc)
  $pen.Dispose(); $ab.Dispose(); $arc.Dispose()

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

# master (256px) also lands at assets/icon.png
Copy-Item (Join-Path $sizesDir "256.png") (Join-Path $assets "icon.png") -Force
Write-Output ("icon master written: {0}" -f (Join-Path $assets "icon.png"))
