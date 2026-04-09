/**
 * 滚动到页面顶部
 * @param behavior 滚动行为，默认为 smooth
 */
export const scrollToTop = (behavior: ScrollBehavior = "smooth") => {
  window.scrollTo({
    top: 0,
    behavior,
  });
};

/**
 * 滚动到指定元素
 * @param element HTML元素
 * @param block 垂直对齐方式
 * @param behavior 滚动行为
 */
export const scrollToElement = (
  element: HTMLElement,
  block: ScrollLogicalPosition = "center",
  behavior: ScrollBehavior = "smooth"
) => {
  element.scrollIntoView({
    behavior,
    block,
  });
};

/**
 * 添加临时高亮效果到元素
 * @param element HTML元素
 * @param shadowStyle 阴影样式
 * @param duration 持续时间（毫秒）
 */
export const addTemporaryHighlight = (
  element: HTMLElement,
  shadowStyle: string = "0 0 0 3px rgba(59, 130, 246, 0.3)",
  duration: number = 2000
) => {
  element.style.boxShadow = shadowStyle;
  setTimeout(() => {
    element.style.boxShadow = "";
  }, duration);
};
