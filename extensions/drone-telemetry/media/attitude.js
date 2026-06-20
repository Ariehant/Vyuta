// Artificial horizon (attitude indicator) rendered with Canvas 2D.
//
// A conventional 2D artificial horizon: a roll/pitch-stabilised sky/ground
// disc with a pitch ladder, a roll scale + pointer, and a fixed aircraft
// reference. Exposed as `window.VyutaAttitude`. (The 3D model viewport from the
// plan is Phase 3 territory; this is the standard flight instrument.)
(function () {
  "use strict";

  const SKY = "#3a7bd5";
  const GROUND = "#7a4a25";
  const LINE = "#ffffff";

  class ArtificialHorizon {
    constructor(canvas) {
      this.canvas = canvas;
      this.ctx = canvas.getContext("2d");
      this.roll = 0; // radians
      this.pitch = 0; // radians
      this.heading = null; // degrees or null
      this.size = 0;
      this.resize();
      window.addEventListener("resize", () => this.resize());
    }

    resize() {
      const dpr = window.devicePixelRatio || 1;
      const wrap = this.canvas.parentElement;
      const rect = wrap.getBoundingClientRect();
      const size = Math.max(120, Math.min(rect.width, rect.height) || 240);
      this.size = size;
      this.canvas.width = Math.round(size * dpr);
      this.canvas.height = Math.round(size * dpr);
      this.canvas.style.width = size + "px";
      this.canvas.style.height = size + "px";
      this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    update(rollRad, pitchRad, heading) {
      this.roll = rollRad || 0;
      this.pitch = pitchRad || 0;
      this.heading = typeof heading === "number" ? heading : null;
    }

    render() {
      const ctx = this.ctx;
      const s = this.size;
      const cx = s / 2;
      const cy = s / 2;
      const R = s / 2 - 6;
      const pxPerDeg = R / 45; // 45° of pitch spans the radius

      ctx.clearRect(0, 0, s, s);
      ctx.save();

      // Clip to the instrument disc.
      ctx.beginPath();
      ctx.arc(cx, cy, R, 0, Math.PI * 2);
      ctx.clip();

      // Stabilised sky/ground.
      ctx.save();
      ctx.translate(cx, cy);
      ctx.rotate(-this.roll);
      const pitchOffset = (this.pitch * 180) / Math.PI * pxPerDeg;
      ctx.translate(0, pitchOffset);

      const big = R * 4;
      ctx.fillStyle = SKY;
      ctx.fillRect(-big, -big, big * 2, big);
      ctx.fillStyle = GROUND;
      ctx.fillRect(-big, 0, big * 2, big);

      // Horizon line.
      ctx.strokeStyle = LINE;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(-big, 0);
      ctx.lineTo(big, 0);
      ctx.stroke();

      // Pitch ladder.
      ctx.font = "10px sans-serif";
      ctx.fillStyle = LINE;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.lineWidth = 1.5;
      for (let deg = -90; deg <= 90; deg += 10) {
        if (deg === 0) continue;
        const y = -deg * pxPerDeg;
        const half = deg % 20 === 0 ? 28 : 16;
        ctx.beginPath();
        ctx.moveTo(-half, y);
        ctx.lineTo(half, y);
        ctx.stroke();
        if (deg % 20 === 0) {
          ctx.fillText(String(Math.abs(deg)), -half - 12, y);
          ctx.fillText(String(Math.abs(deg)), half + 12, y);
        }
      }
      ctx.restore(); // end stabilised layer

      ctx.restore(); // end clip

      // Instrument bezel.
      ctx.strokeStyle = "rgba(255,255,255,0.6)";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(cx, cy, R, 0, Math.PI * 2);
      ctx.stroke();

      // Roll scale (fixed) + roll pointer (moving).
      this.drawRollScale(cx, cy, R);

      // Fixed aircraft reference.
      ctx.strokeStyle = "#ffcc00";
      ctx.lineWidth = 3;
      ctx.beginPath();
      ctx.moveTo(cx - 40, cy);
      ctx.lineTo(cx - 14, cy);
      ctx.moveTo(cx + 14, cy);
      ctx.lineTo(cx + 40, cy);
      ctx.stroke();
      ctx.beginPath();
      ctx.arc(cx, cy, 3, 0, Math.PI * 2);
      ctx.fillStyle = "#ffcc00";
      ctx.fill();
    }

    drawRollScale(cx, cy, R) {
      const ctx = this.ctx;
      const marks = [-60, -45, -30, -20, -10, 0, 10, 20, 30, 45, 60];
      ctx.strokeStyle = LINE;
      ctx.fillStyle = LINE;
      ctx.lineWidth = 1.5;
      for (const m of marks) {
        const a = (-90 + m) * (Math.PI / 180);
        const inner = m === 0 ? R - 14 : R - 8;
        const x1 = cx + Math.cos(a) * R;
        const y1 = cy + Math.sin(a) * R;
        const x2 = cx + Math.cos(a) * inner;
        const y2 = cy + Math.sin(a) * inner;
        ctx.beginPath();
        ctx.moveTo(x1, y1);
        ctx.lineTo(x2, y2);
        ctx.stroke();
      }
      // Moving roll pointer (triangle) at -roll.
      const a = (-90 * Math.PI) / 180 + this.roll * -1;
      const px = cx + Math.cos(a) * (R - 2);
      const py = cy + Math.sin(a) * (R - 2);
      ctx.save();
      ctx.translate(px, py);
      ctx.rotate(a + Math.PI / 2);
      ctx.fillStyle = "#ffcc00";
      ctx.beginPath();
      ctx.moveTo(0, 0);
      ctx.lineTo(-6, -10);
      ctx.lineTo(6, -10);
      ctx.closePath();
      ctx.fill();
      ctx.restore();
    }
  }

  window.VyutaAttitude = ArtificialHorizon;
})();
