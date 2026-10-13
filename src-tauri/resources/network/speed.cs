// Fixed Cloudflare endpoints are selected by the Rust backend. No user files are read.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Net;
using System.Net.Http;
using System.Security.Cryptography;
using System.Threading;
using System.Threading.Tasks;

public sealed class WinEaseSpeedResult {
    public double? mbps;
    public double? latencyMs;
    public double? jitterMs;
    public long bytes;
    public long requestedBytes;
    public double seconds;
    public int completedRequests;
    public string error;
}
public static class WinEaseSpeed {
    sealed class Counters {
        public long bytes, reserved;
        public int completed;
        public string error;
    }
    public static double Rate(long bytes, double seconds) {
        if (bytes <= 0 || seconds <= 0) return 0;
        return bytes * 8.0 / seconds / 1000000.0;
    }
    static HttpRequestMessage Request(string url, int size, bool upload) {
        var request = new HttpRequestMessage(upload ? HttpMethod.Post : HttpMethod.Get,
            url + "?bytes=" + size + "&nonce=" + Guid.NewGuid().ToString("N"));
        request.Headers.TryAddWithoutValidation("Cache-Control", "no-cache, no-store");
        request.Headers.TryAddWithoutValidation("Accept-Encoding", "identity");
        request.Headers.UserAgent.ParseAdd("WinEase/3.5");
        return request;
    }
    public static WinEaseSpeedResult Run(string mode, string url, long cap, int durationMs, int workers, bool systemProxy = true) {
        ServicePointManager.SecurityProtocol = SecurityProtocolType.Tls12;
        ServicePointManager.DefaultConnectionLimit = Math.Max(4, workers);
        ServicePointManager.Expect100Continue = false;
        var handler = new HttpClientHandler {
            AllowAutoRedirect = false,
            AutomaticDecompression = DecompressionMethods.None,
            UseProxy = systemProxy
        };
        if (systemProxy) handler.Proxy = WebRequest.GetSystemWebProxy();
        // Preserve Windows/PAC/TUN routing. Never change network configuration to improve a result.
        using (handler)
        using (var client = new HttpClient(handler)) {
            client.Timeout = TimeSpan.FromSeconds(8);
            if (mode == "latency") return Latency(client, url);
            if (mode != "download" && mode != "upload") throw new ArgumentException("Unknown speed mode");
            if (cap <= 0 || cap > 96L * 1024 * 1024 || workers < 1 || workers > 4 || durationMs < 1 || durationMs > 8000)
                throw new ArgumentException("Invalid measurement limits");
            return Bandwidth(client, url, mode == "upload", cap, durationMs, workers).GetAwaiter().GetResult();
        }
    }
    static WinEaseSpeedResult Latency(HttpClient client, string url) {
        var samples = new List<double>();
        string error = null;
        // A warm-up request is excluded from the latency/jitter samples.
        for (int i = 0; i < 4; i++) {
            var watch = Stopwatch.StartNew();
            try {
                using (var request = Request(url, 0, false))
                using (var response = client.SendAsync(request).GetAwaiter().GetResult()) {
                    response.EnsureSuccessStatusCode();
                    if (i > 0) samples.Add(watch.Elapsed.TotalMilliseconds);
                }
            } catch (Exception ex) { error = ex.GetBaseException().Message; }
        }
        if (samples.Count == 0) return new WinEaseSpeedResult { error = error ?? "No latency samples" };
        double jitter = 0;
        for (int i = 1; i < samples.Count; i++) jitter += Math.Abs(samples[i] - samples[i - 1]);
        jitter = samples.Count > 1 ? jitter / (samples.Count - 1) : 0;
        samples.Sort();
        return new WinEaseSpeedResult { latencyMs = samples[samples.Count / 2], jitterMs = samples.Count > 1 ? (double?)jitter : null, completedRequests = samples.Count };
    }
    static async Task<WinEaseSpeedResult> Bandwidth(HttpClient client, string url, bool upload, long cap, int durationMs, int workers) {
        var count = new Counters();
        var payload = new byte[upload ? 1024 * 1024 : 0];
        if (upload) using (var random = RandomNumberGenerator.Create()) random.GetBytes(payload);
        using (var stop = new CancellationTokenSource(durationMs))
        using (stop.Token.Register(client.CancelPendingRequests)) {
            var watch = Stopwatch.StartNew();
            var tasks = new List<Task>();
            for (int i = 0; i < workers; i++) tasks.Add(Task.Run(async () => {
                int size = 64 * 1024;
                var buffer = new byte[64 * 1024];
                while (!stop.IsCancellationRequested) {
                    int reserved;
                    lock (count) {
                        reserved = (int)Math.Min(size, cap - count.reserved);
                        if (reserved <= 0) break;
                        // Count issued payload even if a request fails, so retries cannot exceed the cap.
                        count.reserved += reserved;
                    }
                    try {
                        using (var request = Request(url, reserved, upload)) {
                            if (upload) {
                                request.Content = new ByteArrayContent(payload, 0, reserved);
                                request.Content.Headers.ContentType = new System.Net.Http.Headers.MediaTypeHeaderValue("application/octet-stream");
                            }
                            using (var response = await client.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, stop.Token)) {
                                response.EnsureSuccessStatusCode();
                                if (upload) {
                                    // Only acknowledged upload requests count towards reported throughput.
                                    Interlocked.Add(ref count.bytes, reserved);
                                } else {
                                    if (response.Content.Headers.ContentEncoding.Count != 0 || response.Content.Headers.ContentLength != reserved)
                                        throw new InvalidOperationException("Unexpected or compressed speed response");
                                    using (var stream = await response.Content.ReadAsStreamAsync()) {
                                        int read;
                                        long received = 0;
                                        while ((read = await stream.ReadAsync(buffer, 0, (int)Math.Min(buffer.Length, reserved - received), stop.Token)) > 0) {
                                            received += read;
                                            Interlocked.Add(ref count.bytes, read);
                                            if (received == reserved) break;
                                        }
                                        if (received != reserved) throw new InvalidOperationException("Incomplete speed response");
                                    }
                                }
                                Interlocked.Increment(ref count.completed);
                            }
                        }
                        size = Math.Min(size * 4, upload ? payload.Length : 4 * 1024 * 1024);
                    } catch (Exception ex) {
                        lock (count) { if (count.error == null) count.error = ex.GetBaseException().Message; }
                        break; // Do not repeatedly retry an unavailable/rate-limited server.
                    }
                }
            }));
            await Task.WhenAll(tasks);
            watch.Stop();
            return new WinEaseSpeedResult {
                mbps = count.bytes > 0 ? (double?)Rate(count.bytes, watch.Elapsed.TotalSeconds) : null,
                bytes = count.bytes, requestedBytes = count.reserved, seconds = watch.Elapsed.TotalSeconds,
                completedRequests = count.completed,
                error = count.bytes > 0 ? null : (count.error ?? "No measurement data received")
            };
        }
    }
}
