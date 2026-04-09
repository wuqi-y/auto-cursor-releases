// 银行卡配置相关的类型定义

export interface BankCardConfig {
  cardNumber: string;
  cardExpiry: string; // MM/YY 格式
  cardCvc: string;
  billingName: string;
  billingCountry: string;
  billingPostalCode: string;
  billingAdministrativeArea: string; // 省份/行政区
  billingLocality: string; // 城市
  billingDependentLocality: string; // 区县
  billingAddressLine1: string; // 详细地址
}

// 批量注册时的银行卡配置数组
export interface BankCardConfigList {
  cards: BankCardConfig[];
}

// 中国省份选项
export interface ProvinceOption {
  value: string;
  label: string;
}

export const CHINA_PROVINCES: ProvinceOption[] = [
  { value: "安徽省 — Anhui Sheng", label: "安徽省 — Anhui Sheng" },
  { value: "澳门 — Macau", label: "澳门 — Macau" },
  { value: "北京市 — Beijing Shi", label: "北京市 — Beijing Shi" },
  { value: "重庆市 — Chongqing Shi", label: "重庆市 — Chongqing Shi" },
  { value: "福建省 — Fujian Sheng", label: "福建省 — Fujian Sheng" },
  { value: "甘肃省 — Gansu Sheng", label: "甘肃省 — Gansu Sheng" },
  { value: "广东省 — Guangdong Sheng", label: "广东省 — Guangdong Sheng" },
  { value: "广西 — Guangxi Zhuangzuzizhiqu", label: "广西壮族自治区 — Guangxi Zhuangzuzizhiqu" },
  { value: "贵州省 — Guizhou Sheng", label: "贵州省 — Guizhou Sheng" },
  { value: "海南省 — Hainan Sheng", label: "海南省 — Hainan Sheng" },
  { value: "河北省 — Hebei Sheng", label: "河北省 — Hebei Sheng" },
  { value: "河南省 — Henan Sheng", label: "河南省 — Henan Sheng" },
  { value: "黑龙江省 — Heilongjiang Sheng", label: "黑龙江省 — Heilongjiang Sheng" },
  { value: "湖北省 — Hubei Sheng", label: "湖北省 — Hubei Sheng" },
  { value: "湖南省 — Hunan Sheng", label: "湖南省 — Hunan Sheng" },
  { value: "吉林省 — Jilin Sheng", label: "吉林省 — Jilin Sheng" },
  { value: "江苏省 — Jiangsu Sheng", label: "江苏省 — Jiangsu Sheng" },
  { value: "江西省 — Jiangxi Sheng", label: "江西省 — Jiangxi Sheng" },
  { value: "辽宁省 — Liaoning Sheng", label: "辽宁省 — Liaoning Sheng" },
  { value: "内蒙古 — Neimenggu Zizhiqu", label: "内蒙古自治区 — Neimenggu Zizhiqu" },
  { value: "宁夏 — Ningxia Huizuzizhiqu", label: "宁夏回族自治区 — Ningxia Huizuzizhiqu" },
  { value: "青海省 — Qinghai Sheng", label: "青海省 — Qinghai Sheng" },
  { value: "山东省 — Shandong Sheng", label: "山东省 — Shandong Sheng" },
  { value: "山西省 — Shanxi Sheng", label: "山西省 — Shanxi Sheng" },
  { value: "陕西省 — Shaanxi Sheng", label: "陕西省 — Shaanxi Sheng" },
  { value: "上海市 — Shanghai Shi", label: "上海市 — Shanghai Shi" },
  { value: "四川省 — Sichuan Sheng", label: "四川省 — Sichuan Sheng" },
  { value: "台湾 — Taiwan", label: "台湾 — Taiwan" },
  { value: "天津市 — Tianjin Shi", label: "天津市 — Tianjin Shi" },
  { value: "西藏 — Xizang Zizhiqu", label: "西藏自治区 — Xizang Zizhiqu" },
  { value: "香港 — Hong Kong", label: "香港 — Hong Kong" },
  { value: "新疆 — Xinjiang Weiwuerzizhiqu", label: "新疆维吾尔自治区 — Xinjiang Weiwuerzizhiqu" },
  { value: "云南省 — Yunnan Sheng", label: "云南省 — Yunnan Sheng" },
  { value: "浙江省 — Zhejiang Sheng", label: "浙江省 — Zhejiang Sheng" },
];

// 默认银行卡配置
export const DEFAULT_BANK_CARD_CONFIG: BankCardConfig = {
  cardNumber: '--',
  cardExpiry: '--',
  cardCvc: '--',
  billingName: '--',
  billingCountry: 'China',
  billingPostalCode: '--',
  billingAdministrativeArea: '--',
  billingLocality: '--',
  billingDependentLocality: '--',
  billingAddressLine1: '--',
};
