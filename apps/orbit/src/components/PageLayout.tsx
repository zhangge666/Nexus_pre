/** 本文件提供 Orbit 工作区页面的统一布局边界，固定标题栏、工具栏和内容区的尺寸关系。 */
import React from "react";

interface PageLayoutProps {
  children: React.ReactNode;
  className?: string;
}

/** 将首个标题栏固定在滚动区外，其余内容交给唯一的页面滚动容器。 */
export function PageLayout({ children, className = "" }: PageLayoutProps): React.JSX.Element {
  const [header, ...content] = React.Children.toArray(children);
  return (
    <div className={`page-layout page-enter ${className}`.trim()}>
      <div className="page-layout-header">{header}</div>
      <div className="page-layout-content">{content}</div>
    </div>
  );
}
