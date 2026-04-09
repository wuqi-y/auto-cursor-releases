import { invoke } from "@tauri-apps/api/core";
import { CursorService } from "../services/cursorService";
import { base64URLEncode, K, sha256 } from "./cursorToken";

/**
 * 根据webToken获取客户端accessToken
 * @param workos_cursor_session_token WorkOS Session Token
 * @returns Promise<any> 返回包含accessToken和refreshToken的对象
 */
export const getClientAccessToken = (workos_cursor_session_token: string) => {
  return new Promise(async (resolve, _reject) => {
    try {
      let verifier = base64URLEncode(K);
      let challenge = base64URLEncode(new Uint8Array(await sha256(verifier)));
      let uuid = crypto.randomUUID();

      // 轮询查token
      let interval = setInterval(() => {
        invoke("trigger_authorization_login_poll", {
          uuid,
          verifier,
        })
          .then((res: any) => {
            console.log(res, "trigger_authorization_login_poll res");
            if (res.success) {
              const data = JSON.parse(res.response_body);
              console.log(data, "access token data");
              resolve(data);
              clearInterval(interval);
            }
          })
          .catch((error) => {
            console.error("轮询获取token失败:", error);
          });
      }, 1000);

      // 20秒后清除定时器
      setTimeout(() => {
        clearInterval(interval);
        resolve(null);
      }, 1000 * 20);

      // 触发授权登录-rust
      await invoke("trigger_authorization_login", {
        uuid,
        challenge,
        workosCursorSessionToken: workos_cursor_session_token,
      });
    } catch (error) {
      console.error("getClientAccessToken error:", error);
      resolve(null);
    }
  });
};

/**
 * 获取当前实际Token
 * @returns Promise<void>
 */
export const fetchActualCurrentToken = async () => {
  try {
    const tokenInfo = await CursorService.getTokenAuto();
    console.log("当前Token信息:", tokenInfo);
    return tokenInfo;
  } catch (error) {
    console.error("获取当前Token失败:", error);
    throw error;
  }
};
