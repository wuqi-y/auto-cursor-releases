// https://api2.cursor.sh/auth/poll?uuid=8c4eaea8-72b3-46d3-89c1-933f67b8cb67&verifier=VJ2d4HTr6Za__D3Un_ralFuZbSPko61djH_RGUAjTt5
// https://cursor.com/cn/loginDeepControl?challenge=IIx3H1m5x8a3qEJS-YRNrU6P3otkBj5RFmshhq2IEXB&uuid=8c4eaea8-72b3-46d3-89c1-933f67b8cb67&mode=login


const Slo = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'
const wlo = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'

function o3(i: Uint8Array, e = !0, t = !1) {
  console.log(i[0], "i");

  const n = t ? Slo : wlo;

  let s = "";
  const r = i.byteLength % 3;

  let o = 0;
  for (; o < i.byteLength - r; o += 3) {

    const a = i[o + 0]
      , u = i[o + 1]
      , d = i[o + 2];

    s += n[a >>> 2],
      s += n[(a << 4 | u >>> 4) & 63],
      s += n[(u << 2 | d >>> 6) & 63],
      s += n[d & 63]
  }
  if (r === 1) {
    const a = i[o + 0];
    s += n[a >>> 2],
      s += n[a << 4 & 63],
      e && (s += "==")
  } else if (r === 2) {
    const a = i[o + 0]
      , u = i[o + 1];
    s += n[a >>> 2],
      s += n[(a << 4 | u >>> 4) & 63],
      s += n[u << 2 & 63],
      e && (s += "=")
  }
  console.log(s, "ss");

  return s
}

// 模拟 CustomBuffer 类 (如果你的环境中已经有 CustomBuffer 类，则不需要这段代码)
class CustomBuffer {
  buffer: Uint8Array;
  byteLength: number;
  constructor(data: Uint8Array) {
    this.buffer = data;
    this.byteLength = data.byteLength;
  }
}

// wrap 函数
function wrap(data: any) {
  // 如果 data 不是 ArrayBuffer，并且支持 Node.js Buffer (这里假设不支持，因为是浏览器环境)
  if (!(data instanceof ArrayBuffer)) {
    // 尝试将 data 转换为 ArrayBufferView，并提取 ArrayBuffer
    if (ArrayBuffer.isView(data)) {
      data = data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength); // 创建 ArrayBuffer 的副本
    } else {
      // 如果 data 既不是 ArrayBuffer 也不是 ArrayBufferView，则抛出错误
      throw new Error("Data must be an ArrayBuffer or ArrayBufferView");
    }
  }

  return new CustomBuffer(data);
}

function base64URLEncode(k: Uint8Array) {
  wrap(k);
  return o3(k, !1, !0)
}

let K = new Uint8Array(32);
K = crypto.getRandomValues(K)

async function sha256(inputString: string) {
  // 检查 'crypto.subtle' 是否可用
  if (!crypto.subtle) {
    // 如果 'crypto.subtle' 不可用，则抛出一个错误
    throw new Error("'crypto.subtle' is not available so webviews will not work. This is likely because the editor is not running in a secure context (https://developer.mozilla.org/en-US/docs/Web/Security/Secure_Contexts).");
  }

  // 将字符串编码为 Uint8Array
  const encoder = new TextEncoder();
  const encodedData = encoder.encode(inputString);

  // 使用 'crypto.subtle.digest' 计算 SHA-256 哈希值
  const hashBuffer = await crypto.subtle.digest("SHA-256", encodedData);

  // 返回 ArrayBuffer 类型的哈希值
  return hashBuffer;
}

(async () => {
  // console.log(K, "K");

  // let verifier = base64URLEncode(K)
  // let challenge = base64URLEncode(new Uint8Array(await sha256(verifier)))

  // let uuid = crypto.randomUUID()

  // console.log(verifier, "verifier");
  // console.log(challenge, "challenge");
  // console.log(uuid, "uuid");
})()


export { K, sha256, base64URLEncode, o3, wrap, CustomBuffer }