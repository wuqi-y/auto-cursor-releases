#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
银行卡自动填写测试脚本
用于独立测试银行卡表单填写功能
"""

import time
import json
import os
from DrissionPage import ChromiumOptions, ChromiumPage
from colorama import init, Fore, Back, Style

# 初始化colorama
init(autoreset=True)

# 表情符号定义
EMOJI = {
    'SUCCESS': '✅',
    'ERROR': '❌',
    'WARNING': '⚠️',
    'INFO': '📝',
    'LOADING': '⏳',
    'ROCKET': '🚀',
    'BANK': '💳',
    'FORM': '📋'
}

class BankCardTester:
    def __init__(self):
        """初始化测试器"""
        # 写死的银行卡测试数据
        self.test_card_info = {
            'cardNumber': '4242424242424242',  # Stripe测试卡号
            'cardExpiry': '12/25',
            'cardCvc': '123',
            'billingName': 'Test User',
            'billingCountry': 'Japan',
            'billingPostalCode': '100-0001',  # 日本邮编格式
            'billingAdministrativeArea': '東京都 — Tokyo',
            'billingLocality': '千代田区',
            'billingDependentLocality': '千代田',
            'billingAddressLine1': 'アイチケン, イチノミヤシ, イシヤマチョウ, 408-1215'
        }
        
        # 配置随机等待时间
        self.config = {
            'input_wait': {'min': 0.5, 'max': 1.5},
            'submit_wait': {'min': 2, 'max': 4},
            'page_wait': {'min': 3, 'max': 6}
        }
        
        print(f"{Fore.GREEN}{EMOJI['BANK']} 银行卡自动填写测试器初始化完成{Style.RESET_ALL}")
        print(f"{Fore.CYAN}{EMOJI['INFO']} 测试卡号: {self.test_card_info['cardNumber'][:4]}****{self.test_card_info['cardNumber'][-4:]}{Style.RESET_ALL}")
        print(f"{Fore.CYAN}{EMOJI['INFO']} 持卡人: {self.test_card_info['billingName']}{Style.RESET_ALL}")

    def get_random_wait_time(self, wait_type):
        """获取随机等待时间"""
        import random
        config = self.config.get(wait_type, {'min': 1, 'max': 2})
        return random.uniform(config['min'], config['max'])

    def create_browser(self):
        """创建浏览器实例"""
        print(f"{Fore.CYAN}{EMOJI['LOADING']} 正在启动浏览器...{Style.RESET_ALL}")
        
        # 设置浏览器选项
        co = ChromiumOptions()
        co.set_argument('--window-size=1280,720')
        co.auto_port()
        co.headless(False)
        
        # 创建浏览器页面
        page = ChromiumPage(addr_or_opts=co)
        
        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 浏览器启动成功{Style.RESET_ALL}")
        return page

    def test_payment_form_fill(self, page, test_url=None):
        """测试银行卡表单填写功能"""
        try:
            if test_url:
                print(f"{Fore.CYAN}{EMOJI['INFO']} 导航到测试页面: {test_url}{Style.RESET_ALL}")
                page.get(test_url)
            else:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 请提供银行卡页面URL{Style.RESET_ALL}")
                return False
                
            time.sleep(self.get_random_wait_time('page_wait'))
            
            print(f"{Fore.CYAN}{EMOJI['INFO']} 当前页面URL: {page.url}{Style.RESET_ALL}")
            print(f"{Fore.CYAN}{EMOJI['INFO']} 页面标题: {page.title}{Style.RESET_ALL}")
            
            # 直接填写银行卡表单
            return self._fill_payment_form(page)
            
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 测试过程中发生错误: {str(e)}{Style.RESET_ALL}")
            return False


    def _fill_payment_form(self, page):
        """填写银行卡信息表单"""
        try:
            print(f"{Fore.CYAN}{EMOJI['FORM']} 开始填写银行卡信息...{Style.RESET_ALL}")
            print(f"{Fore.CYAN}{EMOJI['INFO']} 当前页面URL: {page.url}{Style.RESET_ALL}")
            
            card_info = self.test_card_info
            
            # 等待表单加载
            print(f"{Fore.CYAN}{EMOJI['INFO']} 等待银行卡表单加载...{Style.RESET_ALL}")
            time.sleep(3)
            
            # 查找卡号输入框
            print(f"{Fore.CYAN}{EMOJI['INFO']} 查找卡号输入框 #cardNumber...{Style.RESET_ALL}")
            card_number_input = page.ele("#cardNumber", timeout=15)
            if card_number_input:
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到卡号输入框，开始填写...{Style.RESET_ALL}")
                card_number_input.clear()
                card_number_input.input(card_info['cardNumber'])
                time.sleep(self.get_random_wait_time('input_wait'))
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 卡号填写完成{Style.RESET_ALL}")
            else:
                print(f"{Fore.RED}{EMOJI['ERROR']} 未找到卡号输入框 #cardNumber{Style.RESET_ALL}")
                # 尝试查找其他可能的输入框
                self._debug_page_elements(page)
                return False

            # 填写有效期
            print(f"{Fore.CYAN}{EMOJI['INFO']} 查找有效期输入框 #cardExpiry...{Style.RESET_ALL}")
            card_expiry_input = page.ele("#cardExpiry", timeout=10)
            if card_expiry_input:
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到有效期输入框，开始填写...{Style.RESET_ALL}")
                card_expiry_input.clear()
                card_expiry_input.input(card_info['cardExpiry'])
                time.sleep(self.get_random_wait_time('input_wait'))
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 有效期填写完成{Style.RESET_ALL}")
            else:
                print(f"{Fore.RED}{EMOJI['ERROR']} 未找到有效期输入框 #cardExpiry{Style.RESET_ALL}")
                return False

            # 填写CVC
            print(f"{Fore.CYAN}{EMOJI['INFO']} 查找CVC输入框 #cardCvc...{Style.RESET_ALL}")
            card_cvc_input = page.ele("#cardCvc", timeout=10)
            if card_cvc_input:
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到CVC输入框，开始填写...{Style.RESET_ALL}")
                card_cvc_input.clear()
                card_cvc_input.input(card_info['cardCvc'])
                time.sleep(self.get_random_wait_time('input_wait'))
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} CVC填写完成{Style.RESET_ALL}")
            else:
                print(f"{Fore.RED}{EMOJI['ERROR']} 未找到CVC输入框 #cardCvc{Style.RESET_ALL}")
                return False

            # 填写持卡人姓名
            print(f"{Fore.CYAN}{EMOJI['INFO']} 查找持卡人姓名输入框 #billingName...{Style.RESET_ALL}")
            billing_name_input = page.ele("#billingName", timeout=10)
            if billing_name_input:
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到持卡人姓名输入框，开始填写...{Style.RESET_ALL}")
                billing_name_input.clear()
                billing_name_input.input(card_info['billingName'])
                time.sleep(self.get_random_wait_time('input_wait'))
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 持卡人姓名填写完成{Style.RESET_ALL}")

            # 根据国家决定填写哪些字段
            is_china = card_info['billingCountry'].lower() == 'china'
            print(f"{Fore.CYAN}{EMOJI['INFO']} 检测到国家: {card_info['billingCountry']}, 中国模式: {is_china}{Style.RESET_ALL}")
            
            if is_china:
                # 中国需要填写详细信息
                # 填写邮政编码
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找邮政编码输入框 #billingPostalCode...{Style.RESET_ALL}")
                postal_code_input = page.ele("#billingPostalCode", timeout=10)
                if postal_code_input:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到邮政编码输入框，开始填写...{Style.RESET_ALL}")
                    postal_code_input.clear()
                    postal_code_input.input(card_info['billingPostalCode'])
                    time.sleep(self.get_random_wait_time('input_wait'))
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 邮政编码填写完成{Style.RESET_ALL}")

                # 选择省份
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找省份选择框 #billingAdministrativeArea...{Style.RESET_ALL}")
                province_select = page.ele("#billingAdministrativeArea", timeout=10)
                if province_select:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到省份选择框，开始选择...{Style.RESET_ALL}")
                    try:
                        province_select.select(card_info['billingAdministrativeArea'])
                        time.sleep(self.get_random_wait_time('input_wait'))
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 省份选择完成{Style.RESET_ALL}")
                    except Exception as e:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 省份选择失败: {str(e)}{Style.RESET_ALL}")

                # 填写城市
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找城市输入框 #billingLocality...{Style.RESET_ALL}")
                city_input = page.ele("#billingLocality", timeout=10)
                if city_input:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到城市输入框，开始填写...{Style.RESET_ALL}")
                    city_input.clear()
                    city_input.input(card_info['billingLocality'])
                    time.sleep(self.get_random_wait_time('input_wait'))
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 城市填写完成{Style.RESET_ALL}")

                # 填写区县
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找区县输入框 #billingDependentLocality...{Style.RESET_ALL}")
                district_input = page.ele("#billingDependentLocality", timeout=10)
                if district_input:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到区县输入框，开始填写...{Style.RESET_ALL}")
                    district_input.clear()
                    district_input.input(card_info['billingDependentLocality'])
                    time.sleep(self.get_random_wait_time('input_wait'))
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 区县填写完成{Style.RESET_ALL}")

                # 填写地址
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找地址输入框 #billingAddressLine1...{Style.RESET_ALL}")
                address_input = page.ele("#billingAddressLine1", timeout=10)
                if address_input:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到地址输入框，开始填写...{Style.RESET_ALL}")
                    address_input.clear()
                    address_input.input(card_info['billingAddressLine1'])
                    time.sleep(self.get_random_wait_time('input_wait'))
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 地址填写完成{Style.RESET_ALL}")
            else:
                # 非中国只需要填写地址
                print(f"{Fore.CYAN}{EMOJI['INFO']} 非中国地址，只填写地址字段...{Style.RESET_ALL}")
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找地址输入框 #billingAddressLine1...{Style.RESET_ALL}")
                address_input = page.ele("#billingAddressLine1", timeout=10)
                if address_input:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到地址输入框，开始填写...{Style.RESET_ALL}")
                    address_input.clear()
                    address_input.input(card_info['billingAddressLine1'])
                    time.sleep(3)  # 等待3秒
                    print(f"{Fore.CYAN}{EMOJI['INFO']} 触发Enter事件...{Style.RESET_ALL}")
                    address_input.input('\n')  # 触发Enter事件
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 地址填写完成并触发Enter事件{Style.RESET_ALL}")
                else:
                    print(f"{Fore.RED}{EMOJI['ERROR']} 未找到地址输入框{Style.RESET_ALL}")

            print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 银行卡信息填写完成！{Style.RESET_ALL}")
            
            # 询问是否提交表单
            submit_choice = input(f"\n{Fore.YELLOW}{EMOJI['WARNING']} 是否提交表单？(y/n): {Style.RESET_ALL}")
            if submit_choice.lower() == 'y':
                self._submit_payment_form(page)
            else:
                print(f"{Fore.CYAN}{EMOJI['INFO']} 跳过表单提交，保持页面打开以便检查{Style.RESET_ALL}")
                
            return True
            
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 填写银行卡信息时发生错误: {str(e)}{Style.RESET_ALL}")
            import traceback
            traceback.print_exc()
            return False

    def _submit_payment_form(self, page):
        """提交支付表单"""
        try:
            print(f"{Fore.CYAN}{EMOJI['INFO']} 查找提交按钮...{Style.RESET_ALL}")
            
            # 尝试多种可能的提交按钮选择器
            submit_selectors = [
                "button[type='submit']",
                "input[type='submit']",
                "text:Submit",
                "text:提交",
                "text:Start trial",
                "text:开始试用",
                ".submit-button",
                "#submit-button"
            ]
            
            submit_button = None
            for selector in submit_selectors:
                try:
                    submit_button = page.ele(selector, timeout=2)
                    if submit_button:
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到提交按钮: {selector}{Style.RESET_ALL}")
                        break
                except:
                    continue
                    
            if submit_button:
                print(f"{Fore.CYAN}{EMOJI['INFO']} 点击提交按钮...{Style.RESET_ALL}")
                submit_button.click()
                time.sleep(self.get_random_wait_time('submit_wait'))
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 表单提交完成{Style.RESET_ALL}")
            else:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到提交按钮{Style.RESET_ALL}")
                
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 提交表单时发生错误: {str(e)}{Style.RESET_ALL}")

    def _debug_page_elements(self, page):
        """调试页面元素"""
        try:
            print(f"{Fore.CYAN}{EMOJI['INFO']} 调试页面元素...{Style.RESET_ALL}")
            
            # 查找所有输入框
            all_inputs = page.eles("tag:input")
            print(f"{Fore.CYAN}{EMOJI['INFO']} 找到 {len(all_inputs)} 个输入框{Style.RESET_ALL}")
            
            for i, input_elem in enumerate(all_inputs[:10]):  # 只显示前10个
                try:
                    input_type = input_elem.attr('type') or 'text'
                    input_id = input_elem.attr('id') or 'no-id'
                    input_name = input_elem.attr('name') or 'no-name'
                    input_class = input_elem.attr('class') or 'no-class'
                    print(f"{Fore.CYAN}  输入框 {i+1}: type={input_type}, id={input_id}, name={input_name}, class={input_class}{Style.RESET_ALL}")
                except:
                    print(f"{Fore.YELLOW}  输入框 {i+1}: 无法获取属性{Style.RESET_ALL}")
                    
            # 查找所有选择框
            all_selects = page.eles("tag:select")
            print(f"{Fore.CYAN}{EMOJI['INFO']} 找到 {len(all_selects)} 个选择框{Style.RESET_ALL}")
            
            for i, select_elem in enumerate(all_selects[:5]):  # 只显示前5个
                try:
                    select_id = select_elem.attr('id') or 'no-id'
                    select_name = select_elem.attr('name') or 'no-name'
                    print(f"{Fore.CYAN}  选择框 {i+1}: id={select_id}, name={select_name}{Style.RESET_ALL}")
                except:
                    print(f"{Fore.YELLOW}  选择框 {i+1}: 无法获取属性{Style.RESET_ALL}")
                    
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 调试页面元素时发生错误: {str(e)}{Style.RESET_ALL}")

    def run_test(self, test_url=None):
        """运行测试"""
        print(f"{Fore.GREEN}{EMOJI['ROCKET']} 开始银行卡自动填写测试{Style.RESET_ALL}")
        print("=" * 60)
        
        page = None
        try:
            # 创建浏览器
            page = self.create_browser()
            
            # 运行测试
            success = self.test_payment_form_fill(page, test_url)
            
            if success:
                print(f"\n{Fore.GREEN}{EMOJI['SUCCESS']} 测试完成！银行卡信息填写成功{Style.RESET_ALL}")
            else:
                print(f"\n{Fore.RED}{EMOJI['ERROR']} 测试失败！{Style.RESET_ALL}")
                
            # 保持浏览器打开以便检查
            input(f"\n{Fore.CYAN}{EMOJI['INFO']} 按回车键关闭浏览器...{Style.RESET_ALL}")
            
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 测试运行时发生错误: {str(e)}{Style.RESET_ALL}")
            import traceback
            traceback.print_exc()
            
        finally:
            if page:
                try:
                    page.quit()
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 浏览器已关闭{Style.RESET_ALL}")
                except:
                    pass


def main():
    """主函数"""
    print(f"{Fore.GREEN}{EMOJI['BANK']} 银行卡自动填写测试工具{Style.RESET_ALL}")
    print("=" * 60)
    
    # 创建测试器
    tester = BankCardTester()
    
    # 询问测试URL
    print(f"\n{Fore.CYAN}{EMOJI['INFO']} 请输入银行卡页面URL进行测试{Style.RESET_ALL}")
    test_url = input(f"{Fore.YELLOW}银行卡页面URL: {Style.RESET_ALL}").strip()
    
    if not test_url:
        print(f"{Fore.RED}{EMOJI['ERROR']} URL不能为空{Style.RESET_ALL}")
        return
    
    # 运行测试
    tester.run_test(test_url)


if __name__ == "__main__":
    main()
