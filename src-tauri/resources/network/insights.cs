using System;
using System.Diagnostics;
using System.Net;
using System.Net.Http;
using System.IO;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

public sealed class WinEaseSite {
    public string id, name, url, area, error;
    public bool reachable, ok;
    public int? statusCode;
    public long? latencyMs;
}
public static class WinEaseInsights {
    static HttpClient Client() {
        var handler = new HttpClientHandler {UseProxy=true, Proxy=WebRequest.GetSystemWebProxy(), UseCookies=false, AllowAutoRedirect=true, MaxAutomaticRedirections=3};
        var client = new HttpClient(handler) {Timeout=TimeSpan.FromSeconds(8), MaxResponseContentBufferSize=65536};
        client.DefaultRequestHeaders.UserAgent.ParseAdd("WinEase/3.5 NetworkInsights");
        return client;
    }
    static async Task<WinEaseSite> Probe(HttpClient client, string id, string name, string url, string area) {
        var row = new WinEaseSite {id=id, name=name, url=url, area=area};
        var watch = Stopwatch.StartNew();
        try {
            using (var response = await client.GetAsync(url, HttpCompletionOption.ResponseHeadersRead)) {
                if (response.RequestMessage.RequestUri.Scheme != "https") throw new Exception("网站跳转到未加密连接");
                row.reachable=true; row.statusCode=(int)response.StatusCode;
                row.ok=row.statusCode>=200 && row.statusCode<400;
                row.latencyMs=watch.ElapsedMilliseconds;
            }
        } catch (Exception e) {
            row.error=e is TaskCanceledException ? "连接超时" : e is HttpRequestException ? "无法建立连接，请检查网络或代理" : e.GetBaseException().Message;
        }
        return row;
    }
    public static WinEaseSite[] Sites() {
        using (var client=Client()) {
            return Task.WhenAll(new[] {
                Probe(client,"baidu","百度","https://www.baidu.com/","国内"),
                Probe(client,"bilibili","哔哩哔哩","https://www.bilibili.com/","国内"),
                Probe(client,"qq","腾讯","https://www.qq.com/","国内"),
                Probe(client,"github","GitHub","https://github.com/","国外"),
                Probe(client,"google","Google","https://www.google.com/","国外"),
                Probe(client,"youtube","YouTube","https://www.youtube.com/","国外")
            }).GetAwaiter().GetResult();
        }
    }
    public static string Fetch(string url) {
        using(var client=Client())
        using(var timeout=new CancellationTokenSource(10000))
        using(var response=client.GetAsync(url,HttpCompletionOption.ResponseHeadersRead,timeout.Token).GetAwaiter().GetResult()) {
            response.EnsureSuccessStatusCode();
            if(response.RequestMessage.RequestUri.Scheme!="https") throw new Exception("禁止不安全跳转");
            using(var stream=response.Content.ReadAsStreamAsync().GetAwaiter().GetResult())
            using(var data=new MemoryStream()) {
                var buffer=new byte[4096];int count;
                while((count=stream.ReadAsync(buffer,0,buffer.Length,timeout.Token).GetAwaiter().GetResult())>0) {
                    if(data.Length+count>65536)throw new Exception("响应超过大小上限");
                    data.Write(buffer,0,count);
                }
                return Encoding.UTF8.GetString(data.ToArray());
            }
        }
    }
}
