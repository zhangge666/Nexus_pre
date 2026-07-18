/** 本文件提供 Orbit 在应用内部复用的统一品牌图标。 */
import type React from "react";
import orbitIcon from "../assets/orbit-icon.png";

interface OrbitMarkProps {
  size?: number;
  label?: string;
  className?: string;
}

/** 渲染与桌面应用图标一致的 Orbit 品牌标识。 */
export function OrbitMark({ size = 16, label, className }: OrbitMarkProps): React.JSX.Element {
  return <img className={className} src={orbitIcon} width={size} height={size} alt={label ?? ""} aria-hidden={label ? undefined : true} />;
}
