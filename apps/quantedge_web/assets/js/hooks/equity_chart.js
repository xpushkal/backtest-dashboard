/**
 * EquityChart Hook — renders equity curve with drawdown overlay using Chart.js
 * Loads Chart.js from CDN on first use.
 */
const EquityChart = {
  mounted() {
    this.chart = null
    this.handleEvent("equity_data", (data) => this.renderChart(data))
  },

  async renderChart(data) {
    // Lazy-load Chart.js from CDN
    if (!window.Chart) {
      await this.loadScript("https://cdn.jsdelivr.net/npm/chart.js@4.4.4/dist/chart.umd.min.js")
    }

    const canvas = this.el.querySelector("canvas")
    if (!canvas || !data.labels || data.labels.length === 0) return

    // Destroy existing chart
    if (this.chart) {
      this.chart.destroy()
    }

    const ctx = canvas.getContext("2d")

    this.chart = new Chart(ctx, {
      type: "line",
      data: {
        labels: data.labels,
        datasets: [
          {
            label: "Equity",
            data: data.equity,
            borderColor: "#00d4ff",
            backgroundColor: "rgba(0, 212, 255, 0.05)",
            borderWidth: 2,
            fill: true,
            tension: 0.2,
            pointRadius: 0,
            yAxisID: "y"
          },
          {
            label: "Drawdown %",
            data: data.drawdown.map(d => -Math.abs(d)),
            borderColor: "rgba(255, 61, 61, 0.6)",
            backgroundColor: "rgba(255, 61, 61, 0.1)",
            borderWidth: 1,
            fill: true,
            tension: 0.2,
            pointRadius: 0,
            yAxisID: "y1"
          }
        ]
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        interaction: {
          mode: "index",
          intersect: false,
        },
        plugins: {
          legend: {
            display: true,
            position: "top",
            labels: {
              color: "#8b8fa3",
              font: { family: "Inter", size: 12 },
              usePointStyle: true
            }
          },
          tooltip: {
            backgroundColor: "#1a1a2e",
            titleColor: "#e8eaf0",
            bodyColor: "#8b8fa3",
            borderColor: "#2a2a40",
            borderWidth: 1,
            titleFont: { family: "Inter", weight: "600" },
            bodyFont: { family: "JetBrains Mono", size: 12 },
            callbacks: {
              label: function(context) {
                if (context.datasetIndex === 0) {
                  return `Equity: ₹${Math.round(context.raw).toLocaleString()}`
                }
                return `Drawdown: ${context.raw.toFixed(2)}%`
              }
            }
          }
        },
        scales: {
          x: {
            display: true,
            ticks: {
              color: "#5a5e73",
              font: { family: "Inter", size: 10 },
              maxTicksLimit: 12
            },
            grid: { color: "rgba(30, 30, 46, 0.5)" }
          },
          y: {
            display: true,
            position: "left",
            title: { display: true, text: "Equity (₹)", color: "#8b8fa3" },
            ticks: {
              color: "#8b8fa3",
              font: { family: "JetBrains Mono", size: 11 },
              callback: (v) => `₹${(v/1000).toFixed(0)}K`
            },
            grid: { color: "rgba(30, 30, 46, 0.5)" }
          },
          y1: {
            display: true,
            position: "right",
            title: { display: true, text: "Drawdown %", color: "#ff3d3d" },
            ticks: {
              color: "#ff3d3d",
              font: { family: "JetBrains Mono", size: 11 },
              callback: (v) => `${v.toFixed(1)}%`
            },
            grid: { drawOnChartArea: false }
          }
        }
      }
    })
  },

  loadScript(src) {
    return new Promise((resolve, reject) => {
      const script = document.createElement("script")
      script.src = src
      script.onload = resolve
      script.onerror = reject
      document.head.appendChild(script)
    })
  },

  destroyed() {
    if (this.chart) {
      this.chart.destroy()
    }
  }
}

export { EquityChart }
