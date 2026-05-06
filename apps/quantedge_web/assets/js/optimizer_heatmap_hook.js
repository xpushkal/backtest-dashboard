/**
 * OptimizerHeatmap — Phoenix LiveView JS Hook
 *
 * Renders a 2D heatmap of optimizer results.
 * X/Y axes = 2 selected parameters, cell color = Sharpe ratio.
 * Click cell → pushes event to LiveView.
 */
const OptimizerHeatmap = {
  mounted() {
    this.canvas = document.createElement("canvas");
    this.el.appendChild(this.canvas);
    this.renderHeatmap();
  },

  updated() {
    this.renderHeatmap();
  },

  renderHeatmap() {
    const raw = this.el.dataset.heatmapData;
    if (!raw) return;

    let data;
    try {
      data = JSON.parse(raw);
    } catch (e) {
      return;
    }

    const { x_param, y_param, x_values, y_values, cells } = data;
    if (!x_values || !y_values || !cells) return;

    const cellW = 72;
    const cellH = 48;
    const labelW = 80;
    const labelH = 36;
    const pad = 16;

    const w = labelW + x_values.length * cellW + pad;
    const h = labelH + y_values.length * cellH + pad + 24;

    this.canvas.width = w * 2;
    this.canvas.height = h * 2;
    this.canvas.style.width = w + "px";
    this.canvas.style.height = h + "px";

    const ctx = this.canvas.getContext("2d");
    ctx.scale(2, 2);
    ctx.clearRect(0, 0, w, h);

    // Background
    ctx.fillStyle = "#1a1a2e";
    ctx.fillRect(0, 0, w, h);

    // X-axis labels
    ctx.fillStyle = "#a0a0b8";
    ctx.font = "11px Inter, sans-serif";
    ctx.textAlign = "center";
    for (let i = 0; i < x_values.length; i++) {
      ctx.fillText(
        String(x_values[i]),
        labelW + i * cellW + cellW / 2,
        labelH - 6
      );
    }

    // X-axis title
    ctx.fillStyle = "#8888aa";
    ctx.font = "10px Inter, sans-serif";
    ctx.fillText(x_param || "X", labelW + (x_values.length * cellW) / 2, 10);

    // Y-axis labels + title
    ctx.textAlign = "right";
    ctx.fillStyle = "#a0a0b8";
    ctx.font = "11px Inter, sans-serif";
    for (let j = 0; j < y_values.length; j++) {
      ctx.fillText(
        String(y_values[j]),
        labelW - 8,
        labelH + j * cellH + cellH / 2 + 4
      );
    }

    // Y-axis title
    ctx.save();
    ctx.translate(10, labelH + (y_values.length * cellH) / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillStyle = "#8888aa";
    ctx.font = "10px Inter, sans-serif";
    ctx.textAlign = "center";
    ctx.fillText(y_param || "Y", 0, 0);
    ctx.restore();

    // Build cell lookup
    const cellMap = {};
    for (const c of cells) {
      cellMap[`${c.x}_${c.y}`] = c;
    }

    // Draw cells
    for (let i = 0; i < x_values.length; i++) {
      for (let j = 0; j < y_values.length; j++) {
        const x = labelW + i * cellW;
        const y = labelH + j * cellH;
        const key = `${x_values[i]}_${y_values[j]}`;
        const cell = cellMap[key];

        let color, textColor, label;

        if (!cell || cell.trade_count < 20) {
          color = "#333344";
          textColor = "#666";
          label = "?";
        } else {
          const sharpe = cell.sharpe || 0;
          color = sharpeToColor(sharpe);
          textColor = sharpe > 1.5 ? "#fff" : "#ddd";
          label = sharpe.toFixed(2);
        }

        // Cell fill
        ctx.fillStyle = color;
        ctx.fillRect(x + 1, y + 1, cellW - 2, cellH - 2);

        // Cell border
        ctx.strokeStyle = "#252540";
        ctx.lineWidth = 1;
        ctx.strokeRect(x + 1, y + 1, cellW - 2, cellH - 2);

        // Cell text
        ctx.fillStyle = textColor;
        ctx.font = "bold 12px Inter, sans-serif";
        ctx.textAlign = "center";
        ctx.fillText(label, x + cellW / 2, y + cellH / 2 + 4);

        // PnL sub-label
        if (cell && cell.pnl !== undefined && cell.trade_count >= 20) {
          ctx.fillStyle = cell.pnl >= 0 ? "#4ade80" : "#f87171";
          ctx.font = "9px Inter, sans-serif";
          ctx.fillText(
            formatPnl(cell.pnl),
            x + cellW / 2,
            y + cellH / 2 + 16
          );
        }
      }
    }

    // Click handler
    this.canvas.onclick = (e) => {
      const rect = this.canvas.getBoundingClientRect();
      const mx = (e.clientX - rect.left);
      const my = (e.clientY - rect.top);

      const ci = Math.floor((mx - labelW) / cellW);
      const cj = Math.floor((my - labelH) / cellH);

      if (ci >= 0 && ci < x_values.length && cj >= 0 && cj < y_values.length) {
        const key = `${x_values[ci]}_${y_values[cj]}`;
        const cell = cellMap[key];
        if (cell) {
          this.pushEvent("heatmap_cell_click", {
            x_value: x_values[ci],
            y_value: y_values[cj],
            combo_index: cell.combo_index,
          });
        }
      }
    };

    // Tooltip on hover
    this.canvas.onmousemove = (e) => {
      const rect = this.canvas.getBoundingClientRect();
      const mx = (e.clientX - rect.left);
      const my = (e.clientY - rect.top);

      const ci = Math.floor((mx - labelW) / cellW);
      const cj = Math.floor((my - labelH) / cellH);

      if (ci >= 0 && ci < x_values.length && cj >= 0 && cj < y_values.length) {
        const key = `${x_values[ci]}_${y_values[cj]}`;
        const cell = cellMap[key];
        if (cell) {
          this.canvas.title =
            `${x_param}=${x_values[ci]}, ${y_param}=${y_values[cj]}\n` +
            `Sharpe: ${(cell.sharpe || 0).toFixed(2)}\n` +
            `PnL: ${formatPnl(cell.pnl)}\n` +
            `Trades: ${cell.trade_count}`;
        } else {
          this.canvas.title = "";
        }
        this.canvas.style.cursor = cell ? "pointer" : "default";
      }
    };
  },
};

function sharpeToColor(sharpe) {
  // Diverging: red(-1) → yellow(0) → green(2+)
  const clamped = Math.max(-1, Math.min(3, sharpe));
  const t = (clamped + 1) / 4; // 0..1
  const hue = t * 120; // 0=red, 60=yellow, 120=green
  return `hsl(${hue}, 70%, 38%)`;
}

function formatPnl(pnl) {
  if (pnl === undefined || pnl === null) return "—";
  const abs = Math.abs(pnl);
  if (abs >= 100000) return `₹${(pnl / 100000).toFixed(1)}L`;
  if (abs >= 1000) return `₹${(pnl / 1000).toFixed(1)}K`;
  return `₹${pnl.toFixed(0)}`;
}

export default OptimizerHeatmap;
