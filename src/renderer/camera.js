'use strict';

// 测光窗口采样脚本（T8.3）
// 主进程经 webContents.executeJavaScript('window.__sample()') 调用本文件的 window.__sample。
//
// 约束：不存图、不传网、无任何网络调用；每次采样结束后立即释放摄像头轨。
// 返回：{ ok: true, luma: number }  —— 采样成功，luma 为 64×64 降采样的平均感知亮度（0~255）
//       { ok: false, error: string } —— 无设备 / 被占用 / 权限拒绝等，fail-closed，从不抛异常
//
// 兼容性：requestVideoFrameCallback 存在时用其等待首帧解码；否则以 setTimeout 短延时兜底。
//         两者都叠加一个硬超时竞速，避免隐藏窗口在极端情况下永不回调导致采样永久挂起。

(function () {
  // 全局并发防护：上一次采样未完成时，后续调用复用同一 Promise（防重入）
  let inflight = null;

  // 等待首帧就绪。requestVideoFrameCallback 会保证回调时已有一帧被呈现。
  function waitFirstFrame(video) {
    const frameReady =
      typeof video.requestVideoFrameCallback === 'function'
        ? new Promise(function (resolve) {
            video.requestVideoFrameCallback(function () {
              resolve();
            });
          })
        : new Promise(function (resolve) {
            setTimeout(resolve, 120);
          });

    // 硬超时兜底：无论走哪条分支，最多等待 1500ms，绝不永久挂起。
    const hardTimeout = new Promise(function (resolve) {
      setTimeout(resolve, 1500);
    });

    return Promise.race([frameReady, hardTimeout]);
  }

  async function sample() {
    let stream = null;
    let track = null;
    try {
      stream = await navigator.mediaDevices.getUserMedia({
        video: { width: 320, height: 240 }
      });
      track = stream.getVideoTracks()[0];

      const video = document.createElement('video');
      video.srcObject = stream;
      video.muted = true;
      video.playsInline = true;
      await video.play();
      await waitFirstFrame(video);

      const canvas = document.createElement('canvas');
      canvas.width = 64;
      canvas.height = 64;
      const ctx = canvas.getContext('2d', { willReadFrequently: true });
      ctx.drawImage(video, 0, 0, 64, 64);
      const data = ctx.getImageData(0, 0, 64, 64).data;

      let sum = 0;
      let n = 0;
      for (let i = 0; i < data.length; i += 4) {
        sum += 0.2126 * data[i] + 0.7152 * data[i + 1] + 0.0722 * data[i + 2];
        n++;
      }

      const luma = n ? Math.round((sum / n) * 100) / 100 : 0;
      return { ok: true, luma: luma };
    } catch (err) {
      const name = err && err.name ? err.name : 'Error';
      const message = err && err.message ? err.message : String(err);
      return { ok: false, error: name + ': ' + message };
    } finally {
      // 无论成功失败都释放摄像头：先停主轨，再兜底停掉流内所有轨
      try {
        if (track) track.stop();
      } catch (e) {
        /* ignore */
      }
      try {
        if (stream) {
          stream.getTracks().forEach(function (t) {
            t.stop();
          });
        }
      } catch (e) {
        /* ignore */
      }
    }
  }

  window.__sample = function () {
    if (inflight) return inflight;
    inflight = sample().finally(function () {
      inflight = null;
    });
    return inflight;
  };
})();
