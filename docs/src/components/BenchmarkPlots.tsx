import React, { useEffect, useState } from 'react';

// We must dynamically import Plotly because it relies on window/document,
// which breaks server-side rendering during the Docusaurus build.
export function HeadToHeadPlot() {
  const [Plot, setPlot] = useState<any>(null);

  useEffect(() => {
    import('react-plotly.js').then((module) => {
      setPlot(() => module.default);
    });
  }, []);

  if (!Plot) return <div>Loading plot...</div>;

  return (
    <Plot
      data={[
        {
          x: ['xarray (1 thr)', 'zarr-python', 'zarrs-pipeline', 'Eider (1 thr)'],
          y: [34.6, 13.5, 4.1, 3.0],
          type: 'bar',
          marker: { color: ['#636efa', '#EF553B', '#00cc96', '#ab63fa'] }
        }
      ]}
      layout={{ 
        title: 'Query Latency: California Bounding Box (ms)', 
        yaxis: { title: 'Milliseconds (Lower is better)' },
        paper_bgcolor: 'rgba(0,0,0,0)',
        plot_bgcolor: 'rgba(0,0,0,0)',
        font: { color: 'var(--ifm-font-color-base)' }
      }}
      config={{ responsive: true }}
      style={{ width: '100%', height: '400px' }}
    />
  );
}

export function ScalingPlot() {
  const [Plot, setPlot] = useState<any>(null);

  useEffect(() => {
    import('react-plotly.js').then((module) => {
      setPlot(() => module.default);
    });
  }, []);

  if (!Plot) return <div>Loading plot...</div>;

  return (
    <Plot
      data={[
        {
          x: ['Debug (1 thr)', 'Release (1 thr)', 'SIMD Release (1 thr)'],
          y: [75.0, 37.0, 3.0],
          type: 'scatter',
          mode: 'lines+markers',
          marker: { color: '#ab63fa', size: 12 },
          line: { width: 4 }
        }
      ]}
      layout={{ 
        title: 'Throughput Scaling (Thread = 1)', 
        yaxis: { title: 'Query Time (ms)' },
        paper_bgcolor: 'rgba(0,0,0,0)',
        plot_bgcolor: 'rgba(0,0,0,0)',
        font: { color: 'var(--ifm-font-color-base)' }
      }}
      config={{ responsive: true }}
      style={{ width: '100%', height: '400px' }}
    />
  );
}
