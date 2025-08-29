#!/usr/bin/env python3
"""
BYOIDC WASM Plugin - OAuth Authentication Flow Tester
Automated testing of the complete authentication flow using Selenium
"""

import sys
import time
import json
import subprocess
from urllib.parse import urlparse
from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait
from selenium.webdriver.support import expected_conditions as EC
from selenium.webdriver.firefox.options import Options as FirefoxOptions
from selenium.webdriver.chrome.options import Options as ChromeOptions
from selenium.common.exceptions import TimeoutException, NoSuchElementException
from webdriver_manager.firefox import GeckoDriverManager
from webdriver_manager.chrome import ChromeDriverManager
from selenium.webdriver.firefox.service import Service as FirefoxService
from selenium.webdriver.chrome.service import Service as ChromeService


class AuthFlowTester:
    def __init__(self, browser="chrome", headless=False):  # Changed defaults for better debugging
        self.browser = browser
        self.headless = headless
        self.driver = None
        self.test_results = []
        self.network_logs = []
        
    def setup_driver(self):
        """Initialize the web driver"""
        print(f"🚀 Setting up {self.browser} driver (headless={self.headless})")
        
        if self.browser == "firefox":
            options = FirefoxOptions()
            if self.headless:
                options.add_argument("--headless")
            options.add_argument("--no-sandbox")
            options.add_argument("--disable-dev-shm-usage")
            # Accept insecure certificates (for self-signed certs)
            options.set_preference("security.tls.insecure_fallback_hosts", "odh-gateway.apps-crc.testing")
            options.set_preference("security.cert_pinning.enforcement_level", 0)
            options.set_preference("security.mixed_content.block_active_content", False)
            options.set_preference("security.mixed_content.block_display_content", False)
            options.set_preference("security.cert_pinning.process_headers_from_non_builtin_roots", True)
            options.set_preference("security.fileuri.strict_origin_policy", False)
            options.accept_insecure_certs = True
            service = FirefoxService(GeckoDriverManager().install())
            self.driver = webdriver.Firefox(service=service, options=options)
        else:
            options = ChromeOptions()
            if self.headless:
                options.add_argument("--headless")
            options.add_argument("--no-sandbox")
            options.add_argument("--disable-dev-shm-usage")
            options.add_argument("--ignore-certificate-errors")
            options.add_argument("--ignore-ssl-errors")
            options.add_argument("--allow-running-insecure-content")
            options.add_argument("--ignore-certificate-errors-spki-list")
            options.add_argument("--disable-extensions")
            options.add_argument("--allow-running-insecure-content")
            options.add_argument("--disable-web-security")
            options.add_argument("--ignore-urlfetcher-cert-requests")
            # Enable logging for network activity capture
            options.set_capability('goog:loggingPrefs', {'performance': 'ALL'})
            options.accept_insecure_certs = True
            service = ChromeService(ChromeDriverManager().install())
            self.driver = webdriver.Chrome(service=service, options=options)
            
        self.driver.set_page_load_timeout(60)  # Increased timeout for OAuth redirects
        self.driver.implicitly_wait(15)  # Increased implicit wait
        
        # Enable network logging for Chrome
        if self.browser == "chrome":
            self.driver.execute_cdp_cmd('Network.enable', {})
        
    def log_result(self, test_name, success, message, details=None):
        """Log test result"""
        status = "✅ PASS" if success else "❌ FAIL"
        result = {
            "test": test_name,
            "success": success,
            "message": message,
            "details": details or {},
            "timestamp": time.time()
        }
        self.test_results.append(result)
        print(f"{status} {test_name}: {message}")
        if details:
            for key, value in details.items():
                if key == "page_source_snippet" and len(str(value)) > 200:
                    print(f"   📋 {key}: {str(value)[:200]}...")
                else:
                    print(f"   📋 {key}: {value}")

    def capture_pod_logs(self, step_name):
        """Capture logs from relevant Kubernetes pods"""
        print(f"\n📜 CAPTURING POD LOGS - {step_name}")
        pod_logs = {}
        
        # Define pods to capture logs from
        pods_to_check = [
            {
                "name": "Gateway Pod (istio-proxy with WASM)",
                "namespace": "openshift-ingress", 
                "selector": "gateway.networking.k8s.io/gateway-name=odh-gateway",
                "container": "istio-proxy",
                "lines": 20
            },
            {
                "name": "kube-auth-proxy", 
                "namespace": "openshift-ingress",
                "selector": "app=kube-auth-proxy",
                "container": None,  # Single container
                "lines": 10
            },
            {
                "name": "OAuth Server",
                "namespace": "openshift-authentication",
                "selector": "app=oauth-openshift", 
                "container": None,
                "lines": 5
            }
        ]
        
        for pod_config in pods_to_check:
            try:
                # Build the kubectl command
                cmd = ["oc", "logs", "--tail", str(pod_config["lines"]), "-n", pod_config["namespace"]]
                
                if pod_config["selector"]:
                    cmd.extend(["-l", pod_config["selector"]])
                
                if pod_config["container"]:
                    cmd.extend(["-c", pod_config["container"]])
                    
                # Execute command and capture output
                result = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
                
                if result.returncode == 0:
                    pod_logs[pod_config["name"]] = result.stdout.strip()
                    print(f"   ✅ {pod_config['name']}: {len(result.stdout.strip().split(chr(10)))} lines")
                    
                    # Print key log lines for immediate debugging
                    lines = result.stdout.strip().split('\n')
                    if lines:
                        print(f"      📄 Latest entries:")
                        for line in lines[-3:]:  # Show last 3 lines
                            if line.strip():
                                print(f"         {line.strip()}")
                else:
                    error_msg = result.stderr.strip() or "Unknown error"
                    pod_logs[pod_config["name"]] = f"ERROR: {error_msg}"
                    print(f"   ❌ {pod_config['name']}: {error_msg}")
                    
            except subprocess.TimeoutExpired:
                pod_logs[pod_config["name"]] = "ERROR: Command timeout"
                print(f"   ⏱️ {pod_config['name']}: Command timeout")
            except Exception as e:
                pod_logs[pod_config["name"]] = f"ERROR: {str(e)}"
                print(f"   💥 {pod_config['name']}: {str(e)}")
                
        return pod_logs

    def capture_network_logs(self, step_name):
        """Capture network activity from Chrome DevTools"""
        network_activity = []
        
        if self.browser == "chrome":
            try:
                # Get network logs from Chrome DevTools
                logs = self.driver.get_log('performance')
                
                for log in logs:
                    message = json.loads(log['message'])
                    if message['message']['method'] in ['Network.responseReceived', 'Network.requestWillBeSent']:
                        network_activity.append({
                            'timestamp': log['timestamp'],
                            'method': message['message']['method'],
                            'params': message['message']['params']
                        })
                
                if network_activity:
                    print(f"\n🌐 NETWORK ACTIVITY - {step_name}")
                    for activity in network_activity[-5:]:  # Show last 5 network events
                        method = activity['method']
                        params = activity['params']
                        
                        if method == 'Network.requestWillBeSent':
                            url = params.get('request', {}).get('url', 'Unknown')
                            method_type = params.get('request', {}).get('method', 'Unknown')
                            print(f"   📤 Request: {method_type} {url}")
                            
                        elif method == 'Network.responseReceived':
                            url = params.get('response', {}).get('url', 'Unknown')
                            status = params.get('response', {}).get('status', 'Unknown')
                            print(f"   📥 Response: {status} {url}")
                            
                self.network_logs.extend(network_activity)
                return network_activity
                
            except Exception as e:
                print(f"   ⚠️ Network logging failed: {e}")
                return []
        else:
            print(f"   ℹ️ Network logging only available with Chrome")
            return []
    
    def capture_debug_info(self, step_name):
        """Capture comprehensive debug information"""
        try:
            # Capture pod logs and network activity
            pod_logs = self.capture_pod_logs(step_name)
            network_activity = self.capture_network_logs(step_name)
            
            debug_info = {
                "step": step_name,
                "current_url": self.driver.current_url,
                "page_title": self.driver.title,
                "page_source_length": len(self.driver.page_source),
                "cookies": [{"name": c["name"], "value": c["value"][:50] + "..." if len(c["value"]) > 50 else c["value"]} for c in self.driver.get_cookies()],
            }
            
            # Get page source snippet for key content analysis
            page_source = self.driver.page_source.lower()
            
            # Look for specific patterns that indicate what's happening
            patterns = {
                "login_form": any(pattern in page_source for pattern in ["login", "username", "password", "sign in"]),
                "error_messages": any(pattern in page_source for pattern in ["error", "unauthorized", "forbidden", "failed"]),
                "redirect_indicators": any(pattern in page_source for pattern in ["redirect", "302", "oauth"]),
                "success_indicators": any(pattern in page_source for pattern in ["success", "authenticated", "welcome", "echo"]),
                "wasm_plugin_indicators": any(pattern in page_source for pattern in ["byoidc", "wasm", "plugin"]),
                "kube_auth_proxy_indicators": any(pattern in page_source for pattern in ["kube-auth-proxy", "oauth2"]),
                "openshift_oauth": any(pattern in page_source for pattern in ["openshift", "oauth.openshift", "console-openshift"]),
            }
            
            debug_info["content_patterns"] = patterns
            
            # Extract key lines containing errors or important info
            important_lines = []
            for line in self.driver.page_source.split('\n'):
                line_lower = line.lower().strip()
                if any(keyword in line_lower for keyword in ["error", "unauthorized", "forbidden", "failed", "invalid", "denied"]):
                    important_lines.append(line.strip()[:200])
                if len(important_lines) >= 5:  # Limit to first 5 important lines
                    break

            debug_info["important_content"] = important_lines
            debug_info["pod_logs"] = pod_logs
            debug_info["network_activity"] = network_activity

            print(f"\n🔍 DEBUG INFO - {step_name}")
            print(f"   🌐 URL: {debug_info['current_url']}")
            print(f"   📑 Title: {debug_info['page_title']}")
            print(f"   📏 Page size: {debug_info['page_source_length']} chars")
            print(f"   🍪 Cookies: {len(debug_info['cookies'])} cookies")
            
            print(f"   🔍 Content Analysis:")
            for pattern, found in patterns.items():
                icon = "✅" if found else "❌"
                print(f"      {icon} {pattern}: {found}")
            
            if important_lines:
                print(f"   ⚠️  Important content:")
                for line in important_lines:
                    print(f"      • {line}")
            
            # For small pages, show full content
            if debug_info["page_source_length"] < 1000:
                print(f"   📄 Full page content (small page):")
                lines = self.driver.page_source.strip().split('\n')
                for i, line in enumerate(lines[:20], 1):  # Show first 20 lines max
                    print(f"      {i:2d}: {line.strip()}")
                if len(lines) > 20:
                    print(f"      ... ({len(lines) - 20} more lines)")
            
            return debug_info
            
        except Exception as e:
            print(f"   ⚠️  Debug capture failed: {e}")
            return {"error": str(e)}
        
    def test_initial_redirect(self, gateway_url="https://odh-gateway.apps-crc.testing/"):
        """Test initial access and redirect to login"""
        test_name = "Initial Gateway Access"
        
        try:
            print(f"\n🌐 Navigating to {gateway_url}")
            self.driver.get(gateway_url)
            
            # Wait for page to load and check for redirect (OAuth flows can be slow)
            print("   ⏳ Waiting for page to load and process redirects...")
            time.sleep(8)
            
            # Capture comprehensive debug info
            debug_info = self.capture_debug_info("Initial Page Load")
            
            current_url = self.driver.current_url
            page_title = self.driver.title
            page_source_snippet = self.driver.page_source[:500] + "..." if len(self.driver.page_source) > 500 else self.driver.page_source
            
            details = {
                "original_url": gateway_url,
                "current_url": current_url,
                "page_title": page_title,
                "page_source_length": len(self.driver.page_source),
                "debug_info": debug_info
            }
            
            # Check if we were redirected to OpenShift OAuth
            if "oauth" in current_url.lower() or "login" in current_url.lower():
                self.log_result(test_name, True, "Successfully redirected to OAuth login", details)
                return True
            elif "error" in page_source_snippet.lower():
                self.log_result(test_name, False, "Error page detected", {**details, "page_snippet": page_source_snippet})
                return False
            else:
                self.log_result(test_name, False, f"Unexpected page - no redirect detected", details)
                return False
                
        except TimeoutException:
            self.log_result(test_name, False, "Timeout loading initial page", {"url": gateway_url})
            return False
        except Exception as e:
            self.log_result(test_name, False, f"Error: {str(e)}", {"exception": type(e).__name__})
            return False
    
    def test_login_form(self, username="developer", password="developer"):
        """Test login form submission"""
        test_name = "OAuth Login Form"
        
        try:
            current_url = self.driver.current_url
            print(f"\n🔐 Attempting login at {current_url}")
            
            # Look for common login form elements
            username_selectors = [
                "input[name='username']",
                "input[id='username']", 
                "input[type='text']",
                "#inputUsername",
                ".form-control[name='username']"
            ]
            
            password_selectors = [
                "input[name='password']",
                "input[id='password']",
                "input[type='password']",
                "#inputPassword",
                ".form-control[name='password']"
            ]
            
            submit_selectors = [
                "button[type='submit']",
                "input[type='submit']",
                ".btn-primary",
                "#submit",
                ".login-button"
            ]
            
            username_field = None
            password_field = None
            submit_button = None
            
            # Find username field
            for selector in username_selectors:
                try:
                    username_field = self.driver.find_element(By.CSS_SELECTOR, selector)
                    break
                except NoSuchElementException:
                    continue
                    
            # Find password field  
            for selector in password_selectors:
                try:
                    password_field = self.driver.find_element(By.CSS_SELECTOR, selector)
                    break
                except NoSuchElementException:
                    continue
                    
            # Find submit button
            for selector in submit_selectors:
                try:
                    submit_button = self.driver.find_element(By.CSS_SELECTOR, selector)
                    break
                except NoSuchElementException:
                    continue
            
            if not username_field or not password_field:
                # Maybe it's not a standard form - check page content
                page_content = self.driver.page_source
                details = {
                    "current_url": current_url,
                    "page_title": self.driver.title,
                    "has_username_field": username_field is not None,
                    "has_password_field": password_field is not None,
                    "page_content_snippet": page_content[:1000] + "..." if len(page_content) > 1000 else page_content
                }
                
                if "error" in page_content.lower():
                    self.log_result(test_name, False, "Error detected on login page", details)
                    return False
                elif "unauthorized" in page_content.lower():
                    self.log_result(test_name, False, "Unauthorized error detected", details)
                    return False
                else:
                    self.log_result(test_name, False, "Login form elements not found", details)
                    return False
            
            # Fill and submit the form
            print(f"   📝 Filling username: {username}")
            username_field.clear()
            username_field.send_keys(username)
            
            print(f"   📝 Filling password: {'*' * len(password)}")
            password_field.clear()
            password_field.send_keys(password)
            
            print(f"   🚀 Submitting login form")
            if submit_button:
                submit_button.click()
            else:
                # Try submitting the form via Enter key
                password_field.send_keys("\n")
            
            # Wait for login response and potential redirects
            print("   ⏳ Waiting for login response and redirects...")
            time.sleep(8)
            
            # Capture debug info after login attempt
            debug_info = self.capture_debug_info("Post-Login")
            
            post_login_url = self.driver.current_url
            details = {
                "pre_login_url": current_url,
                "post_login_url": post_login_url,
                "username_used": username,
                "debug_info": debug_info
            }
            
            if post_login_url != current_url:
                self.log_result(test_name, True, "Form submitted successfully - URL changed", details)
                return True
            else:
                self.log_result(test_name, False, "Form submission failed - URL unchanged", details)
                return False
                
        except TimeoutException:
            self.log_result(test_name, False, "Timeout during login", {"current_url": self.driver.current_url})
            return False
        except Exception as e:
            self.log_result(test_name, False, f"Login error: {str(e)}", {"exception": type(e).__name__})
            return False
    
    def test_final_redirect(self, expected_success_indicators=None):
        """Test final redirect back to application"""
        test_name = "Post-Login Redirect"
        
        if expected_success_indicators is None:
            expected_success_indicators = ["echo", "success", "authenticated", "welcome"]
        
        try:
            # Wait for final redirect back to application 
            print("   ⏳ Waiting for final redirect to protected resource...")
            time.sleep(10)
            
            # Capture final debug info
            debug_info = self.capture_debug_info("Final Result")
            
            final_url = self.driver.current_url
            page_title = self.driver.title
            page_source = self.driver.page_source.lower()
            
            details = {
                "final_url": final_url,
                "page_title": page_title,
                "page_length": len(page_source),
                "debug_info": debug_info
            }
            
            # Check for success indicators
            success_found = any(indicator in page_source for indicator in expected_success_indicators)
            error_found = any(error in page_source for error in ["error", "unauthorized", "forbidden", "failed"])
            
            if success_found and not error_found:
                self.log_result(test_name, True, "Authentication successful - reached protected resource", details)
                return True
            elif error_found:
                error_snippet = next((line for line in page_source.split('\n') if any(error in line for error in ["error", "unauthorized", "forbidden"])), "Unknown error")
                self.log_result(test_name, False, f"Authentication failed: {error_snippet[:200]}", details)
                return False
            else:
                self.log_result(test_name, False, "Unclear authentication result", {**details, "page_snippet": page_source[:500]})
                return False
                
        except Exception as e:
            self.log_result(test_name, False, f"Final redirect error: {str(e)}", {"exception": type(e).__name__})
            return False
    
    def run_full_test(self, gateway_url="https://odh-gateway.apps-crc.testing/", username="developer", password="developer"):
        """Run the complete authentication flow test"""
        print("🧪 Starting BYOIDC WASM Plugin Authentication Flow Test")
        print("=" * 60)
        
        try:
            self.setup_driver()
            
            # Test 1: Initial redirect
            if not self.test_initial_redirect(gateway_url):
                print("\n💥 Initial redirect failed - aborting test")
                return False
            
            # Test 2: Login form
            if not self.test_login_form(username, password):
                print("\n💥 Login form submission failed - aborting test") 
                return False
            
            # Test 3: Final redirect
            if not self.test_final_redirect():
                print("\n💥 Final redirect failed")
                return False
            
            print("\n🎉 All tests passed! Authentication flow is working!")
            return True
            
        except Exception as e:
            print(f"\n💥 Test suite error: {str(e)}")
            return False
        finally:
            if self.driver:
                self.driver.quit()
    
    def print_summary(self):
        """Print test summary"""
        print("\n" + "=" * 60)
        print("📊 TEST SUMMARY")
        print("=" * 60)
        
        passed = sum(1 for result in self.test_results if result["success"])
        failed = len(self.test_results) - passed
        
        print(f"Total Tests: {len(self.test_results)}")
        print(f"✅ Passed: {passed}")
        print(f"❌ Failed: {failed}")
        
        if failed > 0:
            print(f"\n🔍 FAILED TESTS:")
            for result in self.test_results:
                if not result["success"]:
                    print(f"   • {result['test']}: {result['message']}")


def main():
    import argparse
    
    parser = argparse.ArgumentParser(description="Test BYOIDC WASM Plugin OAuth Flow")
    parser.add_argument("--url", default="https://odh-gateway.apps-crc.testing/", help="Gateway URL to test")
    parser.add_argument("--username", default="developer", help="Login username")  
    parser.add_argument("--password", default="developer", help="Login password")
    parser.add_argument("--browser", choices=["firefox", "chrome"], default="firefox", help="Browser to use")
    parser.add_argument("--no-headless", action="store_true", help="Run browser in visible mode")
    
    args = parser.parse_args()
    
    tester = AuthFlowTester(browser=args.browser, headless=not args.no_headless)
    
    try:
        success = tester.run_full_test(args.url, args.username, args.password)
        tester.print_summary()
        
        sys.exit(0 if success else 1)
        
    except KeyboardInterrupt:
        print("\n🛑 Test interrupted by user")
        sys.exit(1)
    except Exception as e:
        print(f"\n💥 Unexpected error: {str(e)}")
        sys.exit(1)


if __name__ == "__main__":
    main()
