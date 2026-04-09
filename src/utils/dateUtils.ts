/**
 * 格式化日期字符串为本地化格式
 * @param dateString 日期字符串
 * @param locale 本地化设置，默认为 zh-CN
 * @returns string 格式化后的日期字符串
 */
export const formatDate = (dateString: string, locale: string = "zh-CN"): string => {
  try {
    return new Date(dateString).toLocaleString(locale);
  } catch {
    return dateString;
  }
};

/**
 * 格式化日期为简短格式（年-月-日）
 * @param date 日期对象或日期字符串
 * @returns string 格式化后的日期字符串 (YYYY-MM-DD)
 */
export const formatDateShort = (date: Date | string): string => {
  try {
    const dateObj = typeof date === "string" ? new Date(date) : date;
    return dateObj.toISOString().split("T")[0];
  } catch {
    return "";
  }
};

/**
 * 计算两个日期之间的天数差
 * @param startDate 开始日期
 * @param endDate 结束日期
 * @returns number 天数差
 */
export const getDaysDifference = (startDate: Date, endDate: Date): number => {
  const timeDifference = endDate.getTime() - startDate.getTime();
  return Math.ceil(timeDifference / (1000 * 3600 * 24));
};

/**
 * 获取相对时间描述（如：几天前、几小时前等）
 * @param date 日期
 * @param locale 本地化设置，默认为 zh-CN
 * @returns string 相对时间描述
 */
export const getRelativeTime = (date: Date | string, locale: string = "zh-CN"): string => {
  try {
    const dateObj = typeof date === "string" ? new Date(date) : date;
    const now = new Date();
    const diffInSeconds = Math.floor((now.getTime() - dateObj.getTime()) / 1000);

    if (diffInSeconds < 60) {
      return "刚刚";
    } else if (diffInSeconds < 3600) {
      const minutes = Math.floor(diffInSeconds / 60);
      return `${minutes}分钟前`;
    } else if (diffInSeconds < 86400) {
      const hours = Math.floor(diffInSeconds / 3600);
      return `${hours}小时前`;
    } else if (diffInSeconds < 2592000) {
      const days = Math.floor(diffInSeconds / 86400);
      return `${days}天前`;
    } else {
      return formatDate(dateObj.toISOString(), locale);
    }
  } catch {
    return "未知时间";
  }
};
