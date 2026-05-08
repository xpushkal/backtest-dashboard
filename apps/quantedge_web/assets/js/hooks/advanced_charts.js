/**
 * MonthlyHeatmap Hook — renders a monthly PnL heatmap
 */
const MonthlyHeatmap = {
  mounted() {
    this.handleEvent("heatmap_data", (data) => this.renderHeatmap(data))
    this.pushEvent("request_chart_data", {chart: "heatmap"})
  },

  renderHeatmap(data) {
    const container = this.el
    container.innerHTML = ""

    if (!data.months || data.months.length === 0) {
      container.innerHTML = '<p style="color: #8b8fa3; text-align: center; padding: 2rem;">No monthly data available.</p>'
      return
    }

    // Group by year
    const yearMap = {}
    data.months.forEach(m => {
      const [year, month] = m.label.split("-")
      if (!yearMap[year]) yearMap[year] = {}
      yearMap[year][parseInt(month)] = m.pnl
    })

    const months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"]

    // Build table
    let html = '<table class="data-table" style="text-align:center;">'
    html += '<thead><tr><th>Year</th>'
    months.forEach(m => { html += `<th>${m}</th>` })
    html += '<th>Total</th></tr></thead><tbody>'

    Object.keys(yearMap).sort().forEach(year => {
      html += `<tr><td class="text-mono" style="font-weight:600;">${year}</td>`
      let yearTotal = 0
      for (let i = 1; i <= 12; i++) {
        const val = yearMap[year][i]
        if (val !== undefined) {
          yearTotal += val
          const color = this.pnlColor(val)
          const bg = this.pnlBg(val)
          html += `<td class="text-mono" style="color:${color};background:${bg};padding:8px 4px;border-radius:4px;">${Math.round(val).toLocaleString()}</td>`
        } else {
          html += '<td style="color:#5a5e73;">—</td>'
        }
      }
      const totalColor = this.pnlColor(yearTotal)
      html += `<td class="text-mono" style="color:${totalColor};font-weight:600;">₹${Math.round(yearTotal).toLocaleString()}</td>`
      html += '</tr>'
    })

    html += '</tbody></table>'
    container.innerHTML = html
  },

  pnlColor(val) {
    return val >= 0 ? "#00e676" : "#ff3d3d"
  },

  pnlBg(val) {
    if (val >= 0) {
      const intensity = Math.min(Math.abs(val) / 50000, 1)
      return `rgba(0, 230, 118, ${0.05 + intensity * 0.15})`
    } else {
      const intensity = Math.min(Math.abs(val) / 50000, 1)
      return `rgba(255, 61, 61, ${0.05 + intensity * 0.15})`
    }
  }
}

/**
 * MonteCarloChart Hook — renders Monte Carlo confidence band chart
 */
const MonteCarloChart = {
  mounted() {
    this.chart = null
    this.handleEvent("montecarlo_data", (data) => this.renderChart(data))
    this.pushEvent("request_chart_data", {chart: "montecarlo"})
  },

  async renderChart(data) {
    if (!window.Chart) {
      await loadChartJS()
    }

    const canvas = this.el.querySelector("canvas")
    if (!canvas) return

    if (this.chart) this.chart.destroy()

    const ctx = canvas.getContext("2d")

    const datasets = [
      {
        label: "Actual Equity",
        data: data.actual,
        borderColor: "#00d4ff",
        borderWidth: 2,
        fill: false,
        pointRadius: 0,
        order: 1
      },
      {
        label: "95th Percentile",
        data: data.p95,
        borderColor: "rgba(0, 230, 118, 0.4)",
        backgroundColor: "rgba(0, 230, 118, 0.05)",
        borderWidth: 1,
        fill: "+1",
        pointRadius: 0,
        order: 2
      },
      {
        label: "Median",
        data: data.median,
        borderColor: "rgba(255, 171, 0, 0.6)",
        borderWidth: 1,
        borderDash: [5, 5],
        fill: false,
        pointRadius: 0,
        order: 3
      },
      {
        label: "5th Percentile",
        data: data.p5,
        borderColor: "rgba(255, 61, 61, 0.4)",
        backgroundColor: "rgba(255, 61, 61, 0.05)",
        borderWidth: 1,
        fill: false,
        pointRadius: 0,
        order: 4
      }
    ]

    this.chart = new Chart(ctx, {
      type: "line",
      data: { labels: data.labels, datasets },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        interaction: { mode: "index", intersect: false },
        plugins: {
          legend: {
            position: "top",
            labels: { color: "#8b8fa3", font: { family: "Inter", size: 11 }, usePointStyle: true }
          },
          tooltip: {
            backgroundColor: "#1a1a2e",
            titleColor: "#e8eaf0",
            bodyColor: "#8b8fa3",
            borderColor: "#2a2a40",
            borderWidth: 1
          }
        },
        scales: {
          x: {
            ticks: { color: "#5a5e73", maxTicksLimit: 12 },
            grid: { color: "rgba(30, 30, 46, 0.5)" }
          },
          y: {
            ticks: {
              color: "#8b8fa3",
              font: { family: "JetBrains Mono", size: 11 },
              callback: (v) => `₹${(v/1000).toFixed(0)}K`
            },
            grid: { color: "rgba(30, 30, 46, 0.5)" }
          }
        }
      }
    })
  },

  destroyed() {
    if (this.chart) this.chart.destroy()
  }
}

/**
 * GreeksChart Hook — renders Greeks attribution stacked bar chart
 */
const GreeksChart = {
  mounted() {
    this.chart = null
    this.handleEvent("greeks_data", (data) => this.renderChart(data))
    this.pushEvent("request_chart_data", {chart: "greeks"})
  },

  async renderChart(data) {
    if (!window.Chart) {
      await loadChartJS()
    }

    const canvas = this.el.querySelector("canvas")
    if (!canvas) return

    if (this.chart) this.chart.destroy()

    const ctx = canvas.getContext("2d")

    this.chart = new Chart(ctx, {
      type: "bar",
      data: {
        labels: data.labels,
        datasets: [
          {
            label: "Delta PnL",
            data: data.delta,
            backgroundColor: "rgba(0, 212, 255, 0.7)",
            borderColor: "#00d4ff",
            borderWidth: 1
          },
          {
            label: "Theta PnL",
            data: data.theta,
            backgroundColor: "rgba(0, 230, 118, 0.7)",
            borderColor: "#00e676",
            borderWidth: 1
          },
          {
            label: "Vega PnL",
            data: data.vega,
            backgroundColor: "rgba(179, 136, 255, 0.7)",
            borderColor: "#b388ff",
            borderWidth: 1
          },
          {
            label: "Gamma PnL",
            data: data.gamma,
            backgroundColor: "rgba(255, 171, 0, 0.7)",
            borderColor: "#ffab00",
            borderWidth: 1
          }
        ]
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: {
            position: "top",
            labels: { color: "#8b8fa3", font: { family: "Inter", size: 11 }, usePointStyle: true }
          }
        },
        scales: {
          x: {
            stacked: true,
            ticks: { color: "#5a5e73", maxTicksLimit: 12 },
            grid: { color: "rgba(30, 30, 46, 0.3)" }
          },
          y: {
            stacked: true,
            ticks: {
              color: "#8b8fa3",
              font: { family: "JetBrains Mono", size: 11 }
            },
            grid: { color: "rgba(30, 30, 46, 0.5)" }
          }
        }
      }
    })
  },

  destroyed() {
    if (this.chart) this.chart.destroy()
  }
}

/**
 * WalkForwardChart Hook — renders walk-forward in-sample vs out-of-sample comparison
 */
const WalkForwardChart = {
  mounted() {
    this.chart = null
    this.handleEvent("walkforward_data", (data) => this.renderChart(data))
  },

  async renderChart(data) {
    if (!window.Chart) {
      await loadChartJS()
    }

    const canvas = this.el.querySelector("canvas")
    if (!canvas) return

    if (this.chart) this.chart.destroy()

    const ctx = canvas.getContext("2d")

    this.chart = new Chart(ctx, {
      type: "bar",
      data: {
        labels: data.labels,
        datasets: [
          {
            label: "In-Sample Sharpe",
            data: data.in_sample,
            backgroundColor: "rgba(0, 212, 255, 0.6)",
            borderColor: "#00d4ff",
            borderWidth: 1
          },
          {
            label: "Out-of-Sample Sharpe",
            data: data.out_of_sample,
            backgroundColor: "rgba(0, 230, 118, 0.6)",
            borderColor: "#00e676",
            borderWidth: 1
          }
        ]
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: {
            position: "top",
            labels: { color: "#8b8fa3", font: { family: "Inter", size: 11 }, usePointStyle: true }
          }
        },
        scales: {
          x: {
            ticks: { color: "#5a5e73" },
            grid: { color: "rgba(30, 30, 46, 0.3)" }
          },
          y: {
            title: { display: true, text: "Sharpe Ratio", color: "#8b8fa3" },
            ticks: {
              color: "#8b8fa3",
              font: { family: "JetBrains Mono", size: 11 }
            },
            grid: { color: "rgba(30, 30, 46, 0.5)" }
          }
        }
      }
    })
  },

  destroyed() {
    if (this.chart) this.chart.destroy()
  }
}

/**
 * DailyPnLChart Hook — bar chart of daily PnL
 */
const DailyPnLChart = {
  mounted() {
    this.chart = null
    this.handleEvent("daily_pnl_data", (data) => this.renderChart(data))
    this.pushEvent("request_chart_data", {chart: "daily_pnl"})
  },
  async renderChart(data) {
    if (!window.Chart) await loadChartJS()
    const canvas = this.el.querySelector("canvas")
    if (!canvas) return
    if (this.chart) this.chart.destroy()
    const ctx = canvas.getContext("2d")
    const colors = (data.pnl || []).map(v => v >= 0 ? "rgba(0,230,118,0.7)" : "rgba(255,61,61,0.7)")
    this.chart = new Chart(ctx, {
      type: "bar",
      data: { labels: data.labels, datasets: [{ label: "Daily PnL", data: data.pnl, backgroundColor: colors, borderWidth: 0 }] },
      options: {
        responsive: true, maintainAspectRatio: false,
        plugins: { legend: { display: false } },
        scales: {
          x: { ticks: { color: "#5a5e73", maxTicksLimit: 12 }, grid: { display: false } },
          y: { ticks: { color: "#8b8fa3", callback: v => `₹${(v/1000).toFixed(0)}K` }, grid: { color: "rgba(30,30,46,0.5)" } }
        }
      }
    })
  },
  destroyed() { if (this.chart) this.chart.destroy() }
}

/**
 * DrawdownChart Hook — area chart of drawdown over time
 */
const DrawdownChart = {
  mounted() {
    this.chart = null
    this.handleEvent("drawdown_data", (data) => this.renderChart(data))
    this.pushEvent("request_chart_data", {chart: "drawdown"})
  },
  async renderChart(data) {
    if (!window.Chart) await loadChartJS()
    const canvas = this.el.querySelector("canvas")
    if (!canvas) return
    if (this.chart) this.chart.destroy()
    const ctx = canvas.getContext("2d")
    this.chart = new Chart(ctx, {
      type: "line",
      data: {
        labels: data.labels,
        datasets: [{
          label: "Drawdown %", data: data.drawdown,
          borderColor: "#ff3d3d", backgroundColor: "rgba(255,61,61,0.15)",
          borderWidth: 1.5, fill: true, pointRadius: 0, tension: 0.2
        }]
      },
      options: {
        responsive: true, maintainAspectRatio: false,
        plugins: { legend: { labels: { color: "#8b8fa3" } } },
        scales: {
          x: { ticks: { color: "#5a5e73", maxTicksLimit: 12 }, grid: { color: "rgba(30,30,46,0.3)" } },
          y: { ticks: { color: "#8b8fa3", callback: v => `${v.toFixed(1)}%` }, grid: { color: "rgba(30,30,46,0.5)" } }
        }
      }
    })
  },
  destroyed() { if (this.chart) this.chart.destroy() }
}

/**
 * ReturnsHistogram Hook — histogram of trade PnL distribution
 */
const ReturnsHistogram = {
  mounted() {
    this.chart = null
    this.handleEvent("histogram_data", (data) => this.renderChart(data))
    this.pushEvent("request_chart_data", {chart: "histogram"})
  },
  async renderChart(data) {
    if (!window.Chart) await loadChartJS()
    const canvas = this.el.querySelector("canvas")
    if (!canvas) return
    if (this.chart) this.chart.destroy()
    const ctx = canvas.getContext("2d")
    const colors = (data.labels || []).map(l => parseFloat(l) >= 0 ? "rgba(0,230,118,0.7)" : "rgba(255,61,61,0.7)")
    this.chart = new Chart(ctx, {
      type: "bar",
      data: { labels: data.labels, datasets: [{ label: "Trade Count", data: data.counts, backgroundColor: colors, borderWidth: 0 }] },
      options: {
        responsive: true, maintainAspectRatio: false,
        plugins: { legend: { display: false } },
        scales: {
          x: { ticks: { color: "#5a5e73", maxTicksLimit: 10 }, grid: { display: false } },
          y: { ticks: { color: "#8b8fa3" }, grid: { color: "rgba(30,30,46,0.5)" } }
        }
      }
    })
  },
  destroyed() { if (this.chart) this.chart.destroy() }
}

// Shared Chart.js loader
function loadChartJS() {
  return new Promise((resolve, reject) => {
    if (window.Chart) return resolve()
    const script = document.createElement("script")
    script.src = "https://cdn.jsdelivr.net/npm/chart.js@4.4.4/dist/chart.umd.min.js"
    script.onload = resolve
    script.onerror = reject
    document.head.appendChild(script)
  })
}

export { MonthlyHeatmap, MonteCarloChart, GreeksChart, WalkForwardChart, DailyPnLChart, DrawdownChart, ReturnsHistogram }
