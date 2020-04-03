import React from 'react';
import { withKnobs, text, boolean, number } from "@storybook/addon-knobs";

const magOptions = {
  range: true,
  min: 0,
  max: 1,
  step: 0.05,
};

function controlPoints([x1, y1], [x2, y2], [x3, y3]) {
  let cpfrac = number("CP fraction", 0.35, magOptions)
  let angle_to_prev = Math.atan2(y2 - y1, x2 - x1)
  let angle_to_next = Math.atan2(y2 - y3, x2 - x3)
  // things to adjust and improve
  let angle_to_tangent = (Math.PI + angle_to_next + angle_to_prev) / 2
  let cp2mag = -cpfrac * Math.sqrt((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1))
  let cp3mag = cpfrac * Math.sqrt((x2 - x3) * (x2 - x3) + (y2 - y3) * (y2 - y3))
  // ^^ improve these
  let dx = Math.cos(angle_to_tangent)
  let dy = Math.sin(angle_to_tangent)
  if (angle_to_prev < angle_to_next) {
    dy *= -1
    dx *= -1
  }
  return [[x2 + dx * cp2mag, y2 + dy * cp2mag], [x2 + dx * cp3mag, y2 + dy * cp3mag]]
}

function Curve({points}) {
  if (!points) return <p>No points provided</p>
  function pp([x, y]) {
    return x + ' ' + y
  }
  let d = `M ${pp(points[0])} `
  let [cp2, cp3] = controlPoints(points[0], points[1], points[2])
  d += `C ${pp(points[0])}, ${pp(cp2)}, ${pp(points[1])} `
  d += `C ${pp(cp3)}, ${pp(points[2])}, ${pp(points[2])} `
  return <svg>
    <circle cx={points[0][0]} cy={points[0][1]} r={2} style={{ fill: 'green' }} />
    <circle cx={cp2[0]} cy={cp2[1]} r={2} style={{ fill: 'red' }} />
    <circle cx={cp3[0]} cy={cp3[1]} r={2} style={{ fill: 'blue' }} />
    <path d={d} style={{ stroke: 'black' }} />
  </svg>
}

export default {
  title: 'Curve',
  component: Curve,
  decorators: [withKnobs],
};
const xOptions = {
  range: true,
  min: 0,
  max: 180,
  step: 5,
};

export const CurveDown = () => <Curve points={[[number("p1x", 10, xOptions), 10], [80, 30], [number("p3x", 10, xOptions), 60]]} />;

export const CurveUp = () => <Curve points={[[number("p1x", 10, xOptions), 80], [80, 30], [number("p3x", 10, xOptions), 20]]} />;

export const CurveRight = () => <Curve points={[[10, number("p1y", 10, xOptions)], [30, 80], [60, number("p3y", 10, xOptions)]]} />;

export const CurveLeft = () => <Curve points={[[80, number("p1y", 10, xOptions)], [30, 80], [20, number("p3y", 10, xOptions)]]} />;
