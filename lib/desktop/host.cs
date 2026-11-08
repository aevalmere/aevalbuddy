// The desktop helper: UI Automation and OCR read, SendInput acts, one JSON
// object per line on stdin and stdout. lib/desktop/index.js owns the process
// and is the only caller; its header comment is the contract.
//
// Compiled by Add-Type inside Windows PowerShell 5.1 (see host.ps1), which
// means the C# 5 compiler that ships with .NET Framework: no string
// interpolation, no ?. operator, no expression-bodied members, no out var.
//
// Coordinates are physical virtual-screen pixels. The first thing Run() does
// is ask for per-monitor v2 DPI awareness, so UIA rectangles, GetWindowRect,
// WindowFromPoint, screen capture and SendInput all speak the same unscaled
// units on every monitor.

using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Web.Script.Serialization;
using System.Windows.Automation;
using System.Windows.Automation.Text;

namespace AevalDesktop
{
    public class DesktopError : Exception
    {
        public readonly string Code;
        public DesktopError(string code, string message) : base(message) { Code = code; }
    }

    internal static class Native
    {
        public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);
        public delegate bool MonitorEnumProc(IntPtr hMonitor, IntPtr hdc, IntPtr rect, IntPtr data);
        public delegate IntPtr HookProc(int code, IntPtr wParam, IntPtr lParam);

        [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
        [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
        [StructLayout(LayoutKind.Sequential)] public struct MSG { public IntPtr hwnd; public uint message; public IntPtr wParam; public IntPtr lParam; public uint time; public POINT pt; }
        [StructLayout(LayoutKind.Sequential)] public struct MSLLHOOKSTRUCT { public POINT pt; public uint mouseData; public uint flags; public uint time; public IntPtr extra; }
        [StructLayout(LayoutKind.Sequential)] public struct KBDLLHOOKSTRUCT { public uint vkCode; public uint scanCode; public uint flags; public uint time; public IntPtr extra; }

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        public struct MONITORINFOEX
        {
            public int cbSize;
            public RECT rcMonitor;
            public RECT rcWork;
            public uint dwFlags;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string szDevice;
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
        [StructLayout(LayoutKind.Sequential)]
        public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
        [StructLayout(LayoutKind.Explicit)]
        public struct INPUTUNION { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }
        [StructLayout(LayoutKind.Sequential)]
        public struct INPUT { public uint type; public INPUTUNION u; }

        [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
        [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumWindowsProc cb, IntPtr lParam);
        [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
        [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
        [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder sb, int max);
        [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder sb, int max);
        [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
        [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
        [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int index);
        [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
        [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
        [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
        [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
        [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
        [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
        [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
        [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
        [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
        [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index);
        [DllImport("user32.dll", SetLastError = true)] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
        [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint mapType);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern short VkKeyScan(char ch);
        [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
        [DllImport("user32.dll")] public static extern bool PostThreadMessage(uint thread, uint msg, IntPtr w, IntPtr l);
        [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr h, uint flags);
        [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
        [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr ctx);
        [DllImport("user32.dll")] public static extern IntPtr GetThreadDpiAwarenessContext();
        [DllImport("user32.dll")] public static extern bool AreDpiAwarenessContextsEqual(IntPtr a, IntPtr b);
        [DllImport("user32.dll")] public static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr clip, MonitorEnumProc cb, IntPtr data);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool GetMonitorInfo(IntPtr mon, ref MONITORINFOEX info);
        [DllImport("shcore.dll")] public static extern int GetDpiForMonitor(IntPtr mon, int type, out uint dpiX, out uint dpiY);
        [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out int value, int size);
        [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT value, int size);
        [DllImport("kernel32.dll")] public static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
        [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
        [DllImport("advapi32.dll")] public static extern bool OpenProcessToken(IntPtr proc, uint access, out IntPtr token);
        [DllImport("advapi32.dll")] public static extern bool GetTokenInformation(IntPtr token, int cls, IntPtr info, int len, out int ret);
        [DllImport("advapi32.dll")] public static extern IntPtr GetSidSubAuthorityCount(IntPtr sid);
        [DllImport("advapi32.dll")] public static extern IntPtr GetSidSubAuthority(IntPtr sid, uint index);
        [DllImport("user32.dll", SetLastError = true)] public static extern IntPtr SetWindowsHookEx(int id, HookProc proc, IntPtr mod, uint thread);
        [DllImport("user32.dll")] public static extern bool UnhookWindowsHookEx(IntPtr hook);
        [DllImport("user32.dll")] public static extern IntPtr CallNextHookEx(IntPtr hook, int code, IntPtr w, IntPtr l);
        [DllImport("user32.dll")] public static extern int GetMessage(out MSG msg, IntPtr h, uint min, uint max);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr GetModuleHandle(string name);
        [DllImport("user32.dll")] public static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
        [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr desk);

        public const uint GA_ROOT = 2, GA_ROOTOWNER = 3;
        public const int GWL_EXSTYLE = -20;
        public const int WS_EX_TOOLWINDOW = 0x80, WS_EX_TRANSPARENT = 0x20;
        public const uint WM_CLOSE = 0x10, WM_QUIT = 0x12;
        public const uint INPUT_MOUSE = 0, INPUT_KEYBOARD = 1;
        public const uint KEYEVENTF_EXTENDEDKEY = 1, KEYEVENTF_KEYUP = 2, KEYEVENTF_UNICODE = 4;
        public const uint MOUSEEVENTF_MOVE = 1, MOUSEEVENTF_WHEEL = 0x0800, MOUSEEVENTF_HWHEEL = 0x1000;
        public const uint MOUSEEVENTF_ABSOLUTE = 0x8000, MOUSEEVENTF_VIRTUALDESK = 0x4000;
        public static readonly IntPtr PER_MONITOR_AWARE_V2 = new IntPtr(-4);
        // Marks input as ours ("AEVB"), next to the injected flag Windows sets.
        public static readonly IntPtr EXTRA = new IntPtr(0x41455642);
    }

    internal class RefEntry
    {
        public AutomationElement El;
        public IntPtr Top;
        public int Pid;
        public string Rid;
    }

    // One element's properties, read from a cache request (one round trip
    // for a whole subtree) or live.
    internal class Info
    {
        public string Name = "", Role = "", Value, AutomationId = "", Help = "", Rid;
        public int[] Rect;
        public bool Offscreen, Enabled = true, Focused, Focusable, Password;
        public bool Invoke, ValueP, ValueRO, Toggle, Expand, Select, Scroll, Text, Range, ScrollItem;
        public string ToggleState, ExpandState;
        public bool Selected;
        public int Pid;
        public int ControlTypeId;
    }

    internal class OwnWindow { public long Handle; public bool Block = true; }

    public static class Host
    {
        const int Protocol = 2;
        static string version = "";
        static TextWriter output;
        static readonly object outLock = new object();
        static readonly JavaScriptSerializer json = new JavaScriptSerializer { MaxJsonLength = int.MaxValue, RecursionLimit = 64 };
        static string dpiMode = "unaware";
        static int selfPid;
        static int selfIntegrity;

        static readonly object exLock = new object();
        static HashSet<int> exPids = new HashSet<int>();
        static List<OwnWindow> exWins = new List<OwnWindow>();

        static readonly object refLock = new object();
        static readonly Dictionary<int, RefEntry> refs = new Dictionary<int, RefEntry>();
        static readonly Dictionary<string, int> refByRid = new Dictionary<string, int>();
        static readonly Queue<int> refOrder = new Queue<int>();
        static int nextRef = 1;
        const int MaxRefs = 30000;

        static readonly object cursorLock = new object();
        static Native.POINT? savedCursor;

        static readonly AutomationProperty[] Props = new AutomationProperty[] {
            AutomationElement.NameProperty,
            AutomationElement.ControlTypeProperty,
            AutomationElement.BoundingRectangleProperty,
            AutomationElement.IsOffscreenProperty,
            AutomationElement.IsEnabledProperty,
            AutomationElement.IsPasswordProperty,
            AutomationElement.HasKeyboardFocusProperty,
            AutomationElement.IsKeyboardFocusableProperty,
            AutomationElement.AutomationIdProperty,
            AutomationElement.HelpTextProperty,
            AutomationElement.RuntimeIdProperty,
            AutomationElement.ProcessIdProperty,
            AutomationElement.IsInvokePatternAvailableProperty,
            AutomationElement.IsValuePatternAvailableProperty,
            AutomationElement.IsTogglePatternAvailableProperty,
            AutomationElement.IsExpandCollapsePatternAvailableProperty,
            AutomationElement.IsSelectionItemPatternAvailableProperty,
            AutomationElement.IsScrollPatternAvailableProperty,
            AutomationElement.IsScrollItemPatternAvailableProperty,
            AutomationElement.IsTextPatternAvailableProperty,
            AutomationElement.IsRangeValuePatternAvailableProperty,
            ValuePattern.ValueProperty,
            ValuePattern.IsReadOnlyProperty,
            TogglePattern.ToggleStateProperty,
            ExpandCollapsePattern.ExpandCollapseStateProperty,
            SelectionItemPattern.IsSelectedProperty,
            AutomationElement.LocalizedControlTypeProperty,
        };

        static readonly HashSet<int> InteractiveTypes = new HashSet<int> {
            ControlType.Button.Id, ControlType.CheckBox.Id, ControlType.ComboBox.Id, ControlType.Edit.Id,
            ControlType.Hyperlink.Id, ControlType.ListItem.Id, ControlType.MenuItem.Id, ControlType.RadioButton.Id,
            ControlType.Slider.Id, ControlType.Spinner.Id, ControlType.SplitButton.Id, ControlType.TabItem.Id,
            ControlType.TreeItem.Id, ControlType.DataItem.Id, ControlType.HeaderItem.Id,
        };
        // Containers that carry no meaning without a name. Unnamed ones are
        // left out of the tree and their children move up a level, the way
        // read_page drops anonymous divs.
        static readonly HashSet<int> NoiseTypes = new HashSet<int> {
            ControlType.Pane.Id, ControlType.Group.Id, ControlType.Custom.Id, ControlType.Separator.Id,
            ControlType.Thumb.Id, ControlType.ToolBar.Id, ControlType.StatusBar.Id,
        };
        static readonly HashSet<int> NotFocusTargets = new HashSet<int> {
            ControlType.Pane.Id, ControlType.Group.Id, ControlType.Custom.Id, ControlType.Window.Id,
            ControlType.Document.Id, ControlType.List.Id, ControlType.Tree.Id, ControlType.Table.Id,
            ControlType.DataGrid.Id, ControlType.Tab.Id, ControlType.ToolBar.Id, ControlType.MenuBar.Id,
        };
        // The roles whose Name or Value is page text (spec 1.2, get_page_text).
        static readonly HashSet<int> TextRoles = new HashSet<int> {
            ControlType.Text.Id, ControlType.Edit.Id, ControlType.Document.Id, ControlType.ListItem.Id,
            ControlType.DataItem.Id, ControlType.Hyperlink.Id, ControlType.Header.Id, ControlType.HeaderItem.Id,
            ControlType.TreeItem.Id, ControlType.MenuItem.Id,
        };
        static readonly Dictionary<int, string> roleNames = new Dictionary<int, string>();

        // ------------------------------------------------------------ loop

        public static void Run(string ver)
        {
            SetupDpi();
            version = ver ?? "";
            selfPid = Process.GetCurrentProcess().Id;
            selfIntegrity = Integrity((uint)selfPid);
            exPids.Add(selfPid);
            var stdin = new StreamReader(Console.OpenStandardInput(), new UTF8Encoding(false));
            var sw = new StreamWriter(Console.OpenStandardOutput(), new UTF8Encoding(false));
            sw.AutoFlush = true;
            sw.NewLine = "\n";
            output = sw;

            var ready = new Dictionary<string, object>();
            ready["event"] = "ready";
            ready["protocol"] = Protocol;
            ready["pid"] = selfPid;
            ready["dpi"] = dpiMode;
            ready["integrity"] = selfIntegrity;
            ready["version"] = version;
            Write(ready);

            string line;
            while ((line = stdin.ReadLine()) != null)
            {
                if (line.Trim().Length == 0) continue;
                if (!Handle(line)) break;
            }
            Watch.Stop();
        }

        internal static void Write(Dictionary<string, object> msg)
        {
            string s = json.Serialize(msg);
            lock (outLock) { output.WriteLine(s); }
        }

        class Slot { public object Result; public Exception Error; }

        // Each request runs on its own MTA thread so a provider that hangs
        // (a frozen app answering UIA) costs one request, not the helper.
        // The abandoned thread is a background thread and dies with us.
        static bool Handle(string line)
        {
            object id = null;
            Dictionary<string, object> req;
            try
            {
                req = json.DeserializeObject(line) as Dictionary<string, object>;
                if (req == null) throw new DesktopError("BAD_ARGS", "request is not a JSON object");
                req.TryGetValue("id", out id);
            }
            catch (Exception e)
            {
                Reply(id, null, e is DesktopError ? e : new DesktopError("BAD_ARGS", e.Message), 0);
                return true;
            }
            string op = Str(req, "op", "");
            if (op == "quit") { Reply(id, true, null, 0); return false; }
            var args = Arg(req, "args") as Dictionary<string, object> ?? new Dictionary<string, object>();
            int timeout = Math.Max(100, Int(req, "timeoutMs", 4000));

            var slot = new Slot();
            var sw = Stopwatch.StartNew();
            var t = new Thread(delegate ()
            {
                ThreadDpi();
                try { slot.Result = Dispatch(op, args); }
                catch (Exception e) { slot.Error = e; }
            });
            t.IsBackground = true;
            t.SetApartmentState(ApartmentState.MTA);
            t.Start();
            if (!t.Join(timeout))
            {
                Reply(id, null, new DesktopError("TIMEOUT", op + " did not finish in " + timeout + "ms (the target app may be hung)"), sw.ElapsedMilliseconds);
                return true;
            }
            Reply(id, slot.Result, slot.Error, sw.ElapsedMilliseconds);
            return true;
        }

        static void Reply(object id, object result, Exception error, long ms)
        {
            var msg = new Dictionary<string, object>();
            msg["id"] = id;
            msg["ms"] = ms;
            if (error == null)
            {
                msg["ok"] = true;
                msg["result"] = result;
            }
            else
            {
                msg["ok"] = false;
                var err = new Dictionary<string, object>();
                err["code"] = CodeOf(error);
                err["message"] = error.Message;
                msg["error"] = err;
            }
            try { Write(msg); }
            catch (Exception e)
            {
                // A result that cannot be serialised must still answer the id.
                var fail = new Dictionary<string, object>();
                var err = new Dictionary<string, object>();
                err["code"] = "INTERNAL"; err["message"] = e.Message;
                fail["id"] = id; fail["ok"] = false; fail["error"] = err;
                Write(fail);
            }
        }

        static string CodeOf(Exception e)
        {
            if (e is TargetInvocationException && e.InnerException != null) e = e.InnerException;
            if (e is AggregateException && e.InnerException != null) e = e.InnerException;
            var de = e as DesktopError;
            if (de != null) return de.Code;
            if (e is ElementNotAvailableException) return "STALE_REF";
            if (e is ElementNotEnabledException) return "DISABLED";
            if (e is InvalidOperationException) return "UIA_ERROR";
            if (e is COMException) return "UIA_ERROR";
            if (e is UnauthorizedAccessException) return "ELEVATED_TARGET";
            if (e is ArgumentException || e is FormatException || e is InvalidCastException || e is OverflowException) return "BAD_ARGS";
            return "INTERNAL";
        }

        static object Dispatch(string op, Dictionary<string, object> a)
        {
            switch (op)
            {
                case "ping": return Ping();
                case "config": return Config(a);
                case "listWindows": return ListWindows(a);
                case "foregroundWindow": return ForegroundWindow();
                case "monitors": return Monitors();
                case "focusWindow": return FocusCmd(a);
                case "closeWindow": return CloseCmd(a);
                case "readTree": return Tree(a);
                case "getText": return GetText(a);
                case "elementAt": return At(a);
                case "resolve": return Resolve(a);
                case "focused": return FocusedCmd(a);
                case "ocr": return OcrCmd(a);
                case "capture": return CaptureCmd(a);
                case "click": return Click(a);
                case "hover": return Hover(a);
                case "drag": return Drag(a);
                case "type": return TypeCmd(a);
                case "key": return KeyCmd(a);
                case "scroll": return ScrollCmd(a);
                case "scrollTo": return ScrollTo(a);
                case "invoke": return InvokeCmd(a);
                case "setValue": return SetValueCmd(a);
                case "restoreCursor": return RestoreCursor();
                case "windowStates": return WindowStates(a);
                case "watchInput": return Watch.Set(Bool(a, "on", true));
                case "testUserInput": Watch.Emit(Str(a, "kind", "key"), 0, 0, 0, true); return true;
                default: throw new DesktopError("BAD_ARGS", "unknown op: " + op);
            }
        }

        // ------------------------------------------------------------ args

        static object Arg(Dictionary<string, object> a, string k)
        {
            object v;
            return a != null && a.TryGetValue(k, out v) ? v : null;
        }
        static string Str(Dictionary<string, object> a, string k, string def)
        {
            object v = Arg(a, k);
            return v == null ? def : Convert.ToString(v, System.Globalization.CultureInfo.InvariantCulture);
        }
        static int Int(Dictionary<string, object> a, string k, int def)
        {
            object v = Arg(a, k);
            return v == null ? def : Convert.ToInt32(v, System.Globalization.CultureInfo.InvariantCulture);
        }
        static bool Bool(Dictionary<string, object> a, string k, bool def)
        {
            object v = Arg(a, k);
            return v is bool ? (bool)v : def;
        }
        static int RequireInt(Dictionary<string, object> a, string k)
        {
            if (Arg(a, k) == null) throw new DesktopError("BAD_ARGS", "missing " + k);
            return Int(a, k, 0);
        }

        // Handles travel as 'w' + hex ("w1A2B3C"), the shape listWindows
        // returns; a bare number is accepted too.
        static long ParseHandle(object v)
        {
            if (v == null) return 0;
            var s = v as string;
            if (s == null) return Convert.ToInt64(v);
            s = s.Trim();
            if (s.Length == 0) return 0;
            if (s[0] == 'w' || s[0] == 'W') return Convert.ToInt64(s.Substring(1), 16);
            if (s.StartsWith("0x")) return Convert.ToInt64(s.Substring(2), 16);
            return Convert.ToInt64(s);
        }
        static string HandleStr(IntPtr h) { return "w" + h.ToInt64().ToString("X"); }
        static string RefStr(int r) { return "ref_" + r; }
        static int ParseRef(object v)
        {
            if (v == null) return 0;
            var s = v as string;
            if (s == null) return Convert.ToInt32(v);
            if (s.StartsWith("ref_")) s = s.Substring(4);
            int r;
            if (!int.TryParse(s, out r)) throw new DesktopError("BAD_ARGS", "refs look like ref_12");
            return r;
        }
        static int[] RectArg(Dictionary<string, object> a, string k)
        {
            var o = Arg(a, k);
            var arr = o as object[];
            if (arr != null && arr.Length == 4)
                return new int[] { Convert.ToInt32(arr[0]), Convert.ToInt32(arr[1]), Convert.ToInt32(arr[2]), Convert.ToInt32(arr[3]) };
            var d = o as Dictionary<string, object>;
            if (d != null)
            {
                int w = d.ContainsKey("width") ? Int(d, "width", 0) : Int(d, "w", 0);
                int h = d.ContainsKey("height") ? Int(d, "height", 0) : Int(d, "h", 0);
                return new int[] { Int(d, "x", 0), Int(d, "y", 0), w, h };
            }
            if (o != null) throw new DesktopError("BAD_ARGS", k + " must be [x, y, w, h] or {x, y, width, height}");
            return null;
        }

        // ------------------------------------------------------------ DPI

        static void SetupDpi()
        {
            try
            {
                if (Native.SetProcessDpiAwarenessContext(Native.PER_MONITOR_AWARE_V2)) { dpiMode = "per-monitor-v2"; return; }
            }
            catch (EntryPointNotFoundException) { }
            ThreadDpi();
            try
            {
                if (Native.AreDpiAwarenessContextsEqual(Native.GetThreadDpiAwarenessContext(), Native.PER_MONITOR_AWARE_V2))
                    dpiMode = "per-monitor-v2 (thread)";
            }
            catch (EntryPointNotFoundException) { }
        }

        // The process call fails when powershell.exe already chose an
        // awareness; the thread call still wins for work on that thread, so
        // every request thread makes it.
        internal static void ThreadDpi()
        {
            try { Native.SetThreadDpiAwarenessContext(Native.PER_MONITOR_AWARE_V2); }
            catch (EntryPointNotFoundException) { }
        }

        // ------------------------------------------------------------ windows

        static object Ping()
        {
            var r = new Dictionary<string, object>();
            r["protocol"] = Protocol;
            r["pid"] = selfPid;
            r["dpi"] = dpiMode;
            r["version"] = version;
            r["watching"] = Watch.On;
            lock (refLock) { r["refs"] = refs.Count; }
            return r;
        }

        static object Config(Dictionary<string, object> a)
        {
            lock (exLock)
            {
                exPids = new HashSet<int>();
                exPids.Add(selfPid);
                var pids = Arg(a, "pids") as object[];
                if (pids != null) foreach (var p in pids) if (p != null) exPids.Add(Convert.ToInt32(p));
                exWins = new List<OwnWindow>();
                var wins = Arg(a, "windows") as object[];
                if (wins != null)
                    foreach (var w in wins)
                    {
                        var d = w as Dictionary<string, object>;
                        var ow = new OwnWindow();
                        if (d != null) { ow.Handle = ParseHandle(Arg(d, "handle")); ow.Block = Bool(d, "block", true); }
                        else ow.Handle = ParseHandle(w);
                        if (ow.Handle != 0) exWins.Add(ow);
                    }
                var r = new Dictionary<string, object>();
                r["pids"] = exPids.Count;
                r["windows"] = exWins.Count;
                return r;
            }
        }

        static uint PidOf(IntPtr h)
        {
            uint pid;
            Native.GetWindowThreadProcessId(h, out pid);
            return pid;
        }

        static bool IsExcludedPid(int pid)
        {
            lock (exLock) { return exPids.Contains(pid); }
        }

        static bool IsExcludedWindow(IntPtr h)
        {
            if (h == IntPtr.Zero) return false;
            IntPtr root = Native.GetAncestor(h, Native.GA_ROOT);
            IntPtr owner = Native.GetAncestor(h, Native.GA_ROOTOWNER);
            if (root == IntPtr.Zero) root = h;
            lock (exLock)
            {
                foreach (var w in exWins)
                    if (w.Handle == h.ToInt64() || w.Handle == root.ToInt64() || w.Handle == owner.ToInt64()) return true;
                if (exPids.Contains((int)PidOf(root))) return true;
                if (owner != IntPtr.Zero && exPids.Contains((int)PidOf(owner))) return true;
            }
            return false;
        }

        static void RefuseExcluded(IntPtr h)
        {
            if (IsExcludedWindow(h)) throw new DesktopError("BLOCKED_OWN_WINDOW", "refusing to read or act on AevalBuddy's own window");
        }

        // Any input whose point lands inside one of our visible windows is
        // refused, even when that window is click-through and the click would
        // pass under it: the person sees our surface there, so an agent
        // clicking "through" it is clicking something they cannot see. A
        // window registered with block:false (a full-screen lens) is exempt.
        static void RefuseOwnPoint(int x, int y)
        {
            List<OwnWindow> wins;
            lock (exLock) { wins = exWins; }
            foreach (var w in wins)
            {
                if (!w.Block) continue;
                var h = new IntPtr(w.Handle);
                if (!Native.IsWindow(h) || !Native.IsWindowVisible(h) || Native.IsIconic(h)) continue;
                Native.RECT r;
                if (!Native.GetWindowRect(h, out r)) continue;
                if (x >= r.Left && x < r.Right && y >= r.Top && y < r.Bottom)
                    throw new DesktopError("BLOCKED_OWN_WINDOW", "that point is on AevalBuddy's own window");
            }
            IntPtr hit = Native.WindowFromPoint(new Native.POINT { X = x, Y = y });
            if (hit != IntPtr.Zero && IsExcludedWindow(hit))
            {
                bool exempt = false;
                IntPtr root = Native.GetAncestor(hit, Native.GA_ROOT);
                lock (exLock) { foreach (var w in exWins) if (w.Handle == root.ToInt64() && !w.Block) exempt = true; }
                if (!exempt) throw new DesktopError("BLOCKED_OWN_WINDOW", "that point is on AevalBuddy's own window");
            }
        }

        static string Title(IntPtr h)
        {
            int n = Native.GetWindowTextLength(h);
            if (n <= 0) return "";
            var sb = new StringBuilder(n + 2);
            Native.GetWindowText(h, sb, sb.Capacity);
            return sb.ToString();
        }

        static string ClassOf(IntPtr h)
        {
            var sb = new StringBuilder(256);
            Native.GetClassName(h, sb, sb.Capacity);
            return sb.ToString();
        }

        static int[] WindowRect(IntPtr h)
        {
            Native.RECT r;
            // DWMWA_EXTENDED_FRAME_BOUNDS leaves out the invisible resize border.
            if (Native.DwmGetWindowAttribute(h, 9, out r, Marshal.SizeOf(typeof(Native.RECT))) != 0)
                if (!Native.GetWindowRect(h, out r)) return null;
            return new int[] { r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top };
        }

        static bool IsCloaked(IntPtr h)
        {
            int c;
            return Native.DwmGetWindowAttribute(h, 14, out c, 4) == 0 && c != 0;
        }

        // Mandatory integrity level RID of a process (0x2000 medium, 0x3000
        // high), or -1 when the token cannot be opened, which for a process
        // we can see at all usually means it is above us.
        static int Integrity(uint pid)
        {
            IntPtr proc = Native.OpenProcess(0x1000, false, pid);
            if (proc == IntPtr.Zero) return -1;
            try
            {
                IntPtr token;
                if (!Native.OpenProcessToken(proc, 0x8, out token)) return -1;
                try
                {
                    int len;
                    Native.GetTokenInformation(token, 25, IntPtr.Zero, 0, out len);
                    if (len <= 0) return -1;
                    IntPtr buf = Marshal.AllocHGlobal(len);
                    try
                    {
                        if (!Native.GetTokenInformation(token, 25, buf, len, out len)) return -1;
                        IntPtr sid = Marshal.ReadIntPtr(buf);
                        int count = Marshal.ReadByte(Native.GetSidSubAuthorityCount(sid));
                        return Marshal.ReadInt32(Native.GetSidSubAuthority(sid, (uint)(count - 1)));
                    }
                    finally { Marshal.FreeHGlobal(buf); }
                }
                finally { Native.CloseHandle(token); }
            }
            finally { Native.CloseHandle(proc); }
        }

        static bool IsAbove(uint pid)
        {
            if (pid == 0) return false;
            int il = Integrity(pid);
            return il > selfIntegrity && selfIntegrity >= 0;
        }

        // UIPI silently drops synthetic input and UIA calls from a normal
        // process into a higher-integrity one. Saying so beats a click that
        // reports success and did nothing.
        static void RefuseElevated(IntPtr top)
        {
            if (top == IntPtr.Zero) return;
            if (IsAbove(PidOf(top)))
                throw new DesktopError("ELEVATED_TARGET", "\"" + Title(top) + "\" runs as administrator; Windows blocks input to it from a normal app");
        }

        static string ProcessName(uint pid, Dictionary<uint, string> cache)
        {
            string n;
            if (cache != null && cache.TryGetValue(pid, out n)) return n;
            try { n = Process.GetProcessById((int)pid).ProcessName; } catch { n = ""; }
            if (cache != null) cache[pid] = n;
            return n;
        }

        // UWP apps sit inside ApplicationFrameHost; the process that matters
        // is the one owning the CoreWindow child.
        static uint AppPid(IntPtr h, string cls)
        {
            uint pid = PidOf(h);
            if (cls != "ApplicationFrameWindow") return pid;
            uint found = 0;
            Native.EnumChildWindows(h, delegate (IntPtr c, IntPtr l)
            {
                if (ClassOf(c) == "Windows.UI.Core.CoreWindow") { found = PidOf(c); return false; }
                return true;
            }, IntPtr.Zero);
            return found != 0 ? found : pid;
        }

        static string MonitorDevice(IntPtr h)
        {
            IntPtr mon = Native.MonitorFromWindow(h, 2);
            var mi = new Native.MONITORINFOEX();
            mi.cbSize = Marshal.SizeOf(typeof(Native.MONITORINFOEX));
            return Native.GetMonitorInfo(mon, ref mi) ? mi.szDevice : null;
        }

        static Dictionary<string, object> WinInfo(IntPtr h, Dictionary<uint, string> names, IntPtr fg)
        {
            var d = new Dictionary<string, object>();
            string cls = ClassOf(h);
            uint pid = AppPid(h, cls);
            d["handle"] = HandleStr(h);
            d["title"] = Title(h);
            d["process"] = ProcessName(pid, names);
            d["pid"] = (long)pid;
            d["rect"] = WindowRect(h);
            d["display"] = MonitorDevice(h);
            d["isForeground"] = h == fg;
            d["isMinimized"] = Native.IsIconic(h);
            d["elevated"] = IsAbove(PidOf(h));
            d["className"] = cls;
            return d;
        }

        static bool Listable(IntPtr h, bool includeMinimized)
        {
            if (!Native.IsWindowVisible(h) || IsCloaked(h) || IsExcludedWindow(h)) return false;
            if (!includeMinimized && Native.IsIconic(h)) return false;
            int ex = Native.GetWindowLong(h, Native.GWL_EXSTYLE);
            if ((ex & Native.WS_EX_TOOLWINDOW) != 0) return false;
            if (Native.GetWindowTextLength(h) == 0) return false;
            int[] r = WindowRect(h);
            return r != null && ((r[2] > 0 && r[3] > 0) || Native.IsIconic(h));
        }

        static object ListWindows(Dictionary<string, object> a)
        {
            bool includeMin = Bool(a, "includeMinimized", false);
            var list = new List<object>();
            var names = new Dictionary<uint, string>();
            IntPtr fg = Native.GetAncestor(Native.GetForegroundWindow(), Native.GA_ROOT);
            Native.EnumWindows(delegate (IntPtr h, IntPtr l)
            {
                if (Listable(h, includeMin)) list.Add(WinInfo(h, names, fg));
                return true;
            }, IntPtr.Zero);
            return list;
        }

        // The app the person was in: the foreground window, unless that is
        // AevalBuddy itself (it usually is, at the moment someone asks), in
        // which case the topmost ordinary window under it.
        static IntPtr UserForeground()
        {
            IntPtr fg = Native.GetAncestor(Native.GetForegroundWindow(), Native.GA_ROOT);
            if (fg != IntPtr.Zero && !IsExcludedWindow(fg) && Native.IsWindowVisible(fg)) return fg;
            IntPtr pick = IntPtr.Zero;
            Native.EnumWindows(delegate (IntPtr h, IntPtr l)
            {
                if (!Listable(h, false)) return true;
                pick = h;
                return false;
            }, IntPtr.Zero);
            return pick;
        }

        // Read-only, for the self-check's window gates (PLAN-v2.1 Phase B):
        // the style bits, topmost flag and z-order of the given windows, the
        // first window titled `title`, and the top-level window under
        // `point`. It acts on nothing, so our own windows are allowed.
        static object WindowStates(Dictionary<string, object> a)
        {
            var order = new List<IntPtr>();
            Native.EnumWindows(delegate (IntPtr h, IntPtr l) { order.Add(h); return true; }, IntPtr.Zero);
            var list = new List<object>();
            var hs = Arg(a, "handles") as object[];
            if (hs != null)
            {
                foreach (var v in hs)
                {
                    var h = new IntPtr(ParseHandle(v));
                    var d = new Dictionary<string, object>();
                    d["handle"] = HandleStr(h);
                    d["exists"] = Native.IsWindow(h);
                    d["style"] = Native.GetWindowLong(h, -16);
                    int ex = Native.GetWindowLong(h, Native.GWL_EXSTYLE);
                    d["exStyle"] = ex;
                    d["topmost"] = (ex & 0x8) != 0;
                    d["visible"] = Native.IsWindowVisible(h);
                    d["z"] = order.IndexOf(h);
                    list.Add(d);
                }
            }
            var res = new Dictionary<string, object>();
            res["windows"] = list;
            string title = Str(a, "title", "");
            if (title.Length > 0)
            {
                foreach (var h in order)
                {
                    int n = Native.GetWindowTextLength(h);
                    if (n <= 0 || n > 200) continue;
                    var sb = new StringBuilder(n + 1);
                    Native.GetWindowText(h, sb, sb.Capacity);
                    if (sb.ToString() != title) continue;
                    var d = new Dictionary<string, object>();
                    d["handle"] = HandleStr(h);
                    d["z"] = order.IndexOf(h);
                    d["topmost"] = (Native.GetWindowLong(h, Native.GWL_EXSTYLE) & 0x8) != 0;
                    res["titled"] = d;
                    break;
                }
            }
            var pt = Arg(a, "point") as Dictionary<string, object>;
            if (pt != null)
            {
                // With move, the real pointer goes there first, so the page under
                // it gets its mousemove and sets its own click-through state.
                if (Bool(a, "move", false))
                {
                    MoveTo(RequireInt(pt, "x"), RequireInt(pt, "y"));
                    System.Threading.Thread.Sleep(300);
                }
                IntPtr hit = Native.WindowFromPoint(new Native.POINT { X = RequireInt(pt, "x"), Y = RequireInt(pt, "y") });
                res["atPoint"] = HandleStr(Native.GetAncestor(hit, Native.GA_ROOT));
            }
            return res;
        }

        static object ForegroundWindow()
        {
            IntPtr h = UserForeground();
            if (h == IntPtr.Zero) return null;
            return WinInfo(h, null, Native.GetAncestor(Native.GetForegroundWindow(), Native.GA_ROOT));
        }

        static IntPtr TargetWindow(Dictionary<string, object> a)
        {
            long h = ParseHandle(Arg(a, "windowHandle"));
            IntPtr hwnd = h == 0 ? UserForeground() : new IntPtr(h);
            if (hwnd == IntPtr.Zero) throw new DesktopError("NOT_FOUND", "no window to read");
            if (!Native.IsWindow(hwnd)) throw new DesktopError("NOT_FOUND", "window " + HandleStr(hwnd) + " no longer exists");
            hwnd = Native.GetAncestor(hwnd, Native.GA_ROOT);
            RefuseExcluded(hwnd);
            return hwnd;
        }

        static object Monitors()
        {
            var list = new List<object>();
            Native.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, delegate (IntPtr mon, IntPtr hdc, IntPtr rect, IntPtr data)
            {
                var mi = new Native.MONITORINFOEX();
                mi.cbSize = Marshal.SizeOf(typeof(Native.MONITORINFOEX));
                if (!Native.GetMonitorInfo(mon, ref mi)) return true;
                uint dx = 96, dy = 96;
                try { Native.GetDpiForMonitor(mon, 0, out dx, out dy); } catch (DllNotFoundException) { } catch (EntryPointNotFoundException) { }
                var d = new Dictionary<string, object>();
                d["rect"] = new int[] { mi.rcMonitor.Left, mi.rcMonitor.Top, mi.rcMonitor.Right - mi.rcMonitor.Left, mi.rcMonitor.Bottom - mi.rcMonitor.Top };
                d["work"] = new int[] { mi.rcWork.Left, mi.rcWork.Top, mi.rcWork.Right - mi.rcWork.Left, mi.rcWork.Bottom - mi.rcWork.Top };
                d["dpi"] = (int)dx;
                d["scale"] = dx / 96.0;
                d["primary"] = (mi.dwFlags & 1) != 0;
                d["device"] = mi.szDevice;
                list.Add(d);
                return true;
            }, IntPtr.Zero);
            return list;
        }

        static IntPtr ForegroundRoot()
        {
            return Native.GetAncestor(Native.GetForegroundWindow(), Native.GA_ROOT);
        }

        static bool WaitForeground(IntPtr h)
        {
            for (int i = 0; i < 10; i++)
            {
                if (ForegroundRoot() == h) return true;
                Thread.Sleep(20);
            }
            return false;
        }

        static string FocusWindow(IntPtr h)
        {
            if (Native.IsIconic(h)) { Native.ShowWindow(h, 9); Thread.Sleep(150); }
            if (ForegroundRoot() == h) return "already";
            if (Native.SetForegroundWindow(h) && WaitForeground(h)) return "direct";
            // Windows lets the process that sent the last input take the
            // foreground. A zero-length relative mouse move is input that
            // moves nothing.
            SendMouseRel(0, 0, Native.MOUSEEVENTF_MOVE, 0);
            if (Native.SetForegroundWindow(h) && WaitForeground(h)) return "input";
            IntPtr fg = Native.GetForegroundWindow();
            uint dummy;
            uint fgThread = Native.GetWindowThreadProcessId(fg, out dummy);
            uint me = Native.GetCurrentThreadId();
            bool attached = fgThread != 0 && fgThread != me && Native.AttachThreadInput(me, fgThread, true);
            try
            {
                Native.BringWindowToTop(h);
                Native.SetForegroundWindow(h);
            }
            finally { if (attached) Native.AttachThreadInput(me, fgThread, false); }
            if (WaitForeground(h)) return "attach";
            // Last resort: the foreground lock lets a window come forward
            // while Alt is held. The Alt-down lands in the old window and the
            // Alt-up in the new one, so neither sees a lone Alt tap and no
            // menu bar opens.
            SendInputs(new Native.INPUT[] { KeyInput(0x12, false) });
            try { Native.SetForegroundWindow(h); }
            finally { SendInputs(new Native.INPUT[] { KeyInput(0x12, true) }); }
            return WaitForeground(h) ? "alt" : null;
        }

        static object FocusCmd(Dictionary<string, object> a)
        {
            IntPtr h = new IntPtr(ParseHandle(Arg(a, "handle")));
            if (!Native.IsWindow(h)) throw new DesktopError("NOT_FOUND", "no such window");
            h = Native.GetAncestor(h, Native.GA_ROOT);
            RefuseExcluded(h);
            RefuseElevated(h);
            RefuseSecureDesktop();
            string how = FocusWindow(h);
            var r = new Dictionary<string, object>();
            r["ok"] = how != null;
            r["method"] = how;
            return r;
        }

        static object CloseCmd(Dictionary<string, object> a)
        {
            IntPtr h = new IntPtr(ParseHandle(Arg(a, "handle")));
            if (!Native.IsWindow(h)) throw new DesktopError("NOT_FOUND", "no such window");
            RefuseExcluded(h);
            var r = new Dictionary<string, object>();
            r["ok"] = Native.PostMessage(h, Native.WM_CLOSE, IntPtr.Zero, IntPtr.Zero);
            return r;
        }

        // ------------------------------------------------------------ UIA reads

        static CacheRequest MakeCache(TreeScope scope)
        {
            var cr = new CacheRequest();
            foreach (var p in Props) cr.Add(p);
            cr.TreeScope = scope;
            cr.TreeFilter = Automation.ControlViewCondition;
            cr.AutomationElementMode = AutomationElementMode.Full;
            return cr;
        }

        static string RoleOf(ControlType ct)
        {
            if (ct == null) return "unknown";
            lock (roleNames)
            {
                string n;
                if (roleNames.TryGetValue(ct.Id, out n)) return n;
                n = ct.ProgrammaticName;
                int dot = n.LastIndexOf('.');
                n = (dot >= 0 ? n.Substring(dot + 1) : n).ToLowerInvariant();
                roleNames[ct.Id] = n;
                return n;
            }
        }

        static object Get(AutomationElement el, AutomationProperty p, bool cached)
        {
            try
            {
                object v = cached ? el.GetCachedPropertyValue(p, false) : el.GetCurrentPropertyValue(p, false);
                return v == AutomationElement.NotSupported ? null : v;
            }
            catch (ElementNotAvailableException) { throw; }
            catch (InvalidOperationException) { return null; }
            catch (COMException) { return null; }
        }

        static bool B(AutomationElement el, AutomationProperty p, bool cached)
        {
            object v = Get(el, p, cached);
            return v is bool && (bool)v;
        }

        static string S(AutomationElement el, AutomationProperty p, bool cached)
        {
            object v = Get(el, p, cached);
            return v == null ? "" : v.ToString();
        }

        static int[] ToRect(object rv)
        {
            if (!(rv is System.Windows.Rect)) return null;
            var r = (System.Windows.Rect)rv;
            if (r.IsEmpty || double.IsInfinity(r.Width) || double.IsNaN(r.X)) return null;
            return new int[] { (int)Math.Round(r.X), (int)Math.Round(r.Y), (int)Math.Round(r.Width), (int)Math.Round(r.Height) };
        }

        static Info Read(AutomationElement el, bool cached)
        {
            var i = new Info();
            var ct = Get(el, AutomationElement.ControlTypeProperty, cached) as ControlType;
            i.ControlTypeId = ct == null ? 0 : ct.Id;
            i.Role = RoleOf(ct);
            // A web page's dialog (and a XAML one) is a Pane, Group or Custom
            // whose localized type says dialog. The agent's gate reads the
            // text inside one before a click in it, so it gets its own role.
            if (ct == ControlType.Pane || ct == ControlType.Group || ct == ControlType.Custom)
            {
                string lct = S(el, AutomationElement.LocalizedControlTypeProperty, cached).ToLowerInvariant();
                if (lct == "dialog" || lct == "alert dialog" || lct == "alertdialog") i.Role = "dialog";
            }
            i.Name = S(el, AutomationElement.NameProperty, cached);
            i.Rect = ToRect(Get(el, AutomationElement.BoundingRectangleProperty, cached));
            i.Offscreen = B(el, AutomationElement.IsOffscreenProperty, cached);
            object en = Get(el, AutomationElement.IsEnabledProperty, cached);
            i.Enabled = !(en is bool) || (bool)en;
            i.Password = B(el, AutomationElement.IsPasswordProperty, cached);
            i.Focused = B(el, AutomationElement.HasKeyboardFocusProperty, cached);
            i.Focusable = B(el, AutomationElement.IsKeyboardFocusableProperty, cached);
            i.AutomationId = S(el, AutomationElement.AutomationIdProperty, cached);
            i.Help = S(el, AutomationElement.HelpTextProperty, cached);
            var rid = Get(el, AutomationElement.RuntimeIdProperty, cached) as int[];
            if (rid != null && rid.Length > 0) i.Rid = string.Join(".", Array.ConvertAll(rid, x => x.ToString()));
            object pid = Get(el, AutomationElement.ProcessIdProperty, cached);
            if (pid is int) i.Pid = (int)pid;
            i.Invoke = B(el, AutomationElement.IsInvokePatternAvailableProperty, cached);
            i.ValueP = B(el, AutomationElement.IsValuePatternAvailableProperty, cached);
            i.Toggle = B(el, AutomationElement.IsTogglePatternAvailableProperty, cached);
            i.Expand = B(el, AutomationElement.IsExpandCollapsePatternAvailableProperty, cached);
            i.Select = B(el, AutomationElement.IsSelectionItemPatternAvailableProperty, cached);
            i.Scroll = B(el, AutomationElement.IsScrollPatternAvailableProperty, cached);
            i.ScrollItem = B(el, AutomationElement.IsScrollItemPatternAvailableProperty, cached);
            i.Text = B(el, AutomationElement.IsTextPatternAvailableProperty, cached);
            i.Range = B(el, AutomationElement.IsRangeValuePatternAvailableProperty, cached);
            if (i.ValueP)
            {
                i.Value = S(el, ValuePattern.ValueProperty, cached);
                i.ValueRO = B(el, ValuePattern.IsReadOnlyProperty, cached);
            }
            // Never hand a password to a model, even when a provider leaks it.
            if (i.Password && !string.IsNullOrEmpty(i.Value)) i.Value = "•••";
            if (i.Toggle)
            {
                object ts = Get(el, TogglePattern.ToggleStateProperty, cached);
                if (ts is ToggleState) i.ToggleState = ((ToggleState)ts) == ToggleState.On ? "on" : ((ToggleState)ts) == ToggleState.Off ? "off" : "mixed";
            }
            if (i.Expand)
            {
                object es = Get(el, ExpandCollapsePattern.ExpandCollapseStateProperty, cached);
                if (es is ExpandCollapseState)
                {
                    var s = (ExpandCollapseState)es;
                    i.ExpandState = s == ExpandCollapseState.Expanded ? "expanded" : s == ExpandCollapseState.Collapsed ? "collapsed" : s == ExpandCollapseState.PartiallyExpanded ? "partial" : null;
                }
            }
            if (i.Select) i.Selected = B(el, SelectionItemPattern.IsSelectedProperty, cached);
            return i;
        }

        static bool IsInteractive(Info i)
        {
            if (InteractiveTypes.Contains(i.ControlTypeId)) return true;
            if (i.Invoke || i.Toggle || i.Select || i.Range) return true;
            if (i.ValueP && !i.ValueRO) return true;
            if (i.Expand && i.ExpandState != null) return true;
            // A text editor is the most interactive thing in its window, and
            // Document is otherwise left out with the other containers.
            if (i.ControlTypeId == ControlType.Document.Id && i.Focusable && (i.Text || (i.ValueP && !i.ValueRO))) return true;
            if (i.Focusable && !NotFocusTargets.Contains(i.ControlTypeId) && i.Name.Length > 0) return true;
            return false;
        }

        static bool Intersects(int[] a, int[] b)
        {
            if (a == null || b == null) return true;
            return a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3];
        }

        static int RefFor(AutomationElement el, Info i, IntPtr top)
        {
            lock (refLock)
            {
                int r;
                RefEntry e;
                if (i.Rid != null && refByRid.TryGetValue(i.Rid, out r) && refs.TryGetValue(r, out e))
                {
                    e.El = el;
                    e.Top = top;
                    return r;
                }
                r = nextRef++;
                refs[r] = new RefEntry { El = el, Top = top, Pid = i.Pid, Rid = i.Rid };
                if (i.Rid != null) refByRid[i.Rid] = r;
                refOrder.Enqueue(r);
                while (refOrder.Count > MaxRefs)
                {
                    int old = refOrder.Dequeue();
                    RefEntry oe;
                    if (refs.TryGetValue(old, out oe))
                    {
                        refs.Remove(old);
                        if (oe.Rid != null) refByRid.Remove(oe.Rid);
                    }
                }
                return r;
            }
        }

        // The browser toolset's wording, so a model trained on it knows the
        // recovery.
        static DesktopError Stale(int r)
        {
            return new DesktopError("STALE_REF", "ref_" + r + " is stale or not found on the current page. Re-read the page to get fresh references.");
        }

        static RefEntry GetRef(object refArg)
        {
            int r = ParseRef(refArg);
            if (r <= 0) throw new DesktopError("BAD_ARGS", "missing ref");
            RefEntry e;
            lock (refLock)
            {
                if (!refs.TryGetValue(r, out e)) throw Stale(r);
            }
            if (IsExcludedPid(e.Pid) || IsExcludedWindow(e.Top)) throw new DesktopError("BLOCKED_OWN_WINDOW", "refusing to act on AevalBuddy's own window");
            try { var probe = e.El.Current.ControlType; }
            catch (ElementNotAvailableException) { throw Stale(r); }
            return e;
        }

        static int RefNum(object refArg) { return ParseRef(refArg); }

        static string Clip(string s, int max)
        {
            if (s == null) return null;
            return s.Length <= max ? s : s.Substring(0, max) + "…";
        }

        static Dictionary<string, object> NodeDict(Info i, int r, int parent, int depth, int valueMax)
        {
            var d = new Dictionary<string, object>();
            d["ref"] = RefStr(r);
            d["parentRef"] = parent > 0 ? RefStr(parent) : null;
            d["depth"] = depth;
            d["role"] = i.Role;
            d["name"] = Clip(i.Name, 400);
            if (!string.IsNullOrEmpty(i.Value)) d["value"] = Clip(i.Value, valueMax);
            d["rect"] = i.Rect;
            var states = new List<string>();
            if (i.Focused) states.Add("focused");
            if (!i.Enabled) states.Add("disabled");
            if (i.ToggleState == "on") states.Add("checked");
            if (i.ToggleState == "mixed") states.Add("mixed");
            if (i.ExpandState == "expanded") states.Add("expanded");
            if (i.ExpandState == "collapsed") states.Add("collapsed");
            if (i.Selected) states.Add("selected");
            if (i.Password) states.Add("password");
            if (i.Offscreen) states.Add("offscreen");
            if (i.ValueP && i.ValueRO) states.Add("readonly");
            d["states"] = states;
            var patterns = new List<string>();
            if (i.Invoke) patterns.Add("invoke");
            if (i.ValueP) patterns.Add("value");
            if (i.Toggle) patterns.Add("toggle");
            if (i.Expand) patterns.Add("expandcollapse");
            if (i.Select) patterns.Add("selectionitem");
            if (i.Range) patterns.Add("rangevalue");
            if (i.Scroll) patterns.Add("scroll");
            if (i.ScrollItem) patterns.Add("scrollitem");
            if (i.Text) patterns.Add("text");
            d["patterns"] = patterns;
            d["automationId"] = i.AutomationId.Length > 0 ? Clip(i.AutomationId, 120) : null;
            if (i.Help.Length > 0) d["helpText"] = Clip(i.Help, 200);
            if (IsInteractive(i)) d["interactive"] = true;
            return d;
        }

        class Walk
        {
            public string Filter;
            public int MaxDepth, MaxNodes, ValueMax;
            public bool Cached, Lazy;
            public IntPtr Top;
            public int[] WinRect;
            public int Visited;
            public bool Truncated;
            public List<object> Nodes = new List<object>();
            public Func<AutomationElement, AutomationElementCollection> Children;
            // For getText: called per kept node instead of building a tree;
            // returns false to skip the node's children.
            public Func<AutomationElement, Info, int, string, bool> OnNode;
        }

        static void Visit(Walk w, AutomationElement el, int parentRef, int depth, bool isRoot, string parentName)
        {
            if (w.Truncated) return;
            w.Visited++;
            Info i;
            try { i = Read(el, w.Cached); }
            catch (ElementNotAvailableException) { return; }

            bool interactive = IsInteractive(i);
            // IsOffscreen lies in some WPF apps, so a rect outside the window
            // counts as offscreen too.
            bool visible = !i.Offscreen && (i.Rect == null || (i.Rect[2] > 0 && i.Rect[3] > 0 && Intersects(i.Rect, w.WinRect)));
            if (!visible && !i.Offscreen && i.Rect != null) i.Offscreen = true;
            bool emit;
            if (isRoot) emit = true;
            else
            {
                bool noise = NoiseTypes.Contains(i.ControlTypeId) && i.Name.Length == 0 && string.IsNullOrEmpty(i.Value) && !interactive && i.Role != "dialog";
                // A button's text child repeats the button's name.
                bool echo = i.ControlTypeId == ControlType.Text.Id && i.Name.Length > 0 && i.Name == parentName && !interactive;
                // Chromium marks unnamed wrapper groups invokable; listing them
                // gives the model a column of "group [ref_7]" to guess at.
                bool bare = NoiseTypes.Contains(i.ControlTypeId) && i.Name.Length == 0 && string.IsNullOrEmpty(i.Value);
                if (w.Filter == "interactive") emit = interactive && visible && !bare;
                else if (w.Filter == "visible") emit = !noise && !echo && visible;
                else emit = !noise && !echo;
            }

            int myRef = parentRef;
            int childDepth = depth;
            string myName = parentName;
            bool descend = true;
            if (emit)
            {
                myRef = RefFor(el, i, w.Top);
                if (w.OnNode != null) descend = w.OnNode(el, i, myRef, parentName);
                else w.Nodes.Add(NodeDict(i, myRef, parentRef, depth, w.ValueMax));
                childDepth = depth + 1;
                myName = i.Name;
                if (w.Nodes.Count >= w.MaxNodes) { w.Truncated = true; return; }
            }
            if (!descend || childDepth > w.MaxDepth) return;
            // Content scrolled out of view is most of a long web page and has
            // nothing on screen to act on.
            if (!isRoot && i.Offscreen && w.Filter != "all" && w.Lazy) return;
            AutomationElementCollection kids;
            try { kids = w.Children(el); }
            catch (ElementNotAvailableException) { return; }
            catch (InvalidOperationException) { return; }
            if (kids == null) return;
            foreach (AutomationElement k in kids)
            {
                Visit(w, k, myRef, childDepth, false, myName);
                if (w.Truncated) return;
            }
        }

        static readonly object genLock = new object();
        static readonly Dictionary<long, int[]> generations = new Dictionary<long, int[]>();
        static readonly Queue<KeyValuePair<long, AutomationElement>> watched = new Queue<KeyValuePair<long, AutomationElement>>();
        static readonly Dictionary<long, StructureChangedEventHandler> handlers = new Dictionary<long, StructureChangedEventHandler>();
        const int MaxWatched = 6;

        // generation bumps on a structure-changed event, so a caller holding
        // refs knows the tree it read is no longer the tree on screen. The
        // first read of a window subscribes; the least recent of more than
        // six is unsubscribed.
        static int Generation(IntPtr hwnd, AutomationElement root, out bool firstRead)
        {
            long key = hwnd.ToInt64();
            lock (genLock)
            {
                int[] g;
                if (generations.TryGetValue(key, out g)) { firstRead = false; return g[0]; }
                firstRead = true;
                g = new int[] { 1 };
                generations[key] = g;
                var gen = g;
                StructureChangedEventHandler h = delegate (object s, StructureChangedEventArgs e) { Interlocked.Increment(ref gen[0]); };
                try
                {
                    Automation.AddStructureChangedEventHandler(root, TreeScope.Subtree, h);
                    handlers[key] = h;
                    watched.Enqueue(new KeyValuePair<long, AutomationElement>(key, root));
                }
                catch (Exception) { }
                while (watched.Count > MaxWatched)
                {
                    var old = watched.Dequeue();
                    StructureChangedEventHandler oh;
                    if (handlers.TryGetValue(old.Key, out oh))
                    {
                        handlers.Remove(old.Key);
                        generations.Remove(old.Key);
                        var el = old.Value;
                        ThreadPool.QueueUserWorkItem(delegate (object st)
                        {
                            try { Automation.RemoveStructureChangedEventHandler(el, oh); } catch (Exception) { }
                        });
                    }
                }
                return g[0];
            }
        }

        // 'subtree' asks UIA for the whole window in one cross-process call,
        // which is what keeps a Chrome or Explorer window well under a
        // second. It cannot stop early, so 'walk' (one call per container,
        // stops at maxNodes, skips offscreen subtrees) is there for
        // pathological trees.
        static Walk RunWalk(Dictionary<string, object> a, string filter, Func<AutomationElement, Info, int, string, bool> onNode, out int generation)
        {
            IntPtr hwnd;
            AutomationElement start = null;
            object refArg = Arg(a, "ref");
            if (refArg != null)
            {
                var e = GetRef(refArg);
                hwnd = e.Top;
                start = e.El;
            }
            else hwnd = TargetWindow(a);
            var w = new Walk();
            w.Filter = filter;
            w.MaxDepth = Math.Max(0, Int(a, "depth", 15));
            w.MaxNodes = Math.Max(1, Int(a, "maxNodes", 3000));
            w.ValueMax = Math.Max(16, Int(a, "valueMax", 300));
            w.Top = hwnd;
            w.WinRect = WindowRect(hwnd);
            w.OnNode = onNode;
            string strategy = Str(a, "strategy", "subtree");

            AutomationElement winEl = AutomationElement.FromHandle(hwnd);
            bool firstRead;
            generation = Generation(hwnd, winEl, out firstRead);
            if (start == null) start = winEl;

            int[] waits = new int[] { 300, 700 };
            for (int attempt = 0; attempt <= waits.Length; attempt++)
            {
                if (strategy == "walk")
                {
                    w.Lazy = true;
                    var cr = MakeCache(TreeScope.Element);
                    using (cr.Activate())
                    {
                        AutomationElement root = start.GetUpdatedCache(cr);
                        w.Cached = true;
                        w.Children = delegate (AutomationElement e) { return e.FindAll(TreeScope.Children, Automation.ControlViewCondition); };
                        Visit(w, root, 0, 0, true, "");
                    }
                }
                else
                {
                    var cr = MakeCache(TreeScope.Subtree);
                    AutomationElement root = start.GetUpdatedCache(cr);
                    w.Cached = true;
                    w.Children = delegate (AutomationElement e) { return e.CachedChildren; };
                    Visit(w, root, 0, 0, true, "");
                }
                // Chromium, Electron and Firefox build their tree the first
                // time a client asks, so a first read of a window can come
                // back nearly empty. Retried after 300ms, then 700ms more.
                if (!firstRead || w.Visited >= 30 || attempt == waits.Length) break;
                Thread.Sleep(waits[attempt]);
                w.Nodes.Clear();
                w.Visited = 0;
                w.Truncated = false;
            }
            return w;
        }

        static object Tree(Dictionary<string, object> a)
        {
            string filter = Str(a, "filter", "visible");
            if (filter != "interactive" && filter != "all" && filter != "visible") throw new DesktopError("BAD_ARGS", "filter must be 'visible', 'interactive' or 'all'");
            var sw = Stopwatch.StartNew();
            int gen;
            var w = RunWalk(a, filter, null, out gen);
            var r = new Dictionary<string, object>();
            r["window"] = WinInfo(w.Top, null, ForegroundRoot());
            r["windowHandle"] = HandleStr(w.Top);
            r["filter"] = filter;
            r["nodes"] = w.Nodes;
            r["visited"] = w.Visited;
            r["truncated"] = w.Truncated;
            r["generation"] = gen;
            r["readMs"] = sw.ElapsedMilliseconds;
            return r;
        }

        static int[] Union(System.Windows.Rect[] rects)
        {
            if (rects == null || rects.Length == 0) return null;
            double l = double.MaxValue, t = double.MaxValue, rr = double.MinValue, b = double.MinValue;
            foreach (var r in rects)
            {
                if (r.IsEmpty) continue;
                l = Math.Min(l, r.Left); t = Math.Min(t, r.Top); rr = Math.Max(rr, r.Right); b = Math.Max(b, r.Bottom);
            }
            if (l == double.MaxValue) return null;
            return new int[] { (int)Math.Round(l), (int)Math.Round(t), (int)Math.Round(rr - l), (int)Math.Round(b - t) };
        }

        static Dictionary<string, object> Span(string text, int[] rect, int r, string source)
        {
            var d = new Dictionary<string, object>();
            d["text"] = text;
            d["rect"] = rect;
            d["ref"] = r > 0 ? RefStr(r) : null;
            d["source"] = source;
            return d;
        }

        // One span per visible line, with that line's rectangle, so Find can
        // draw on the exact words.
        static bool LineSpans(AutomationElement el, int r, int[] clip, List<Dictionary<string, object>> spans, ref int chars, int maxChars)
        {
            TextPattern tp;
            try { tp = el.GetCurrentPattern(TextPattern.Pattern) as TextPattern; }
            catch (InvalidOperationException) { return false; }
            if (tp == null) return false;
            int got = 0;
            foreach (var vis in tp.GetVisibleRanges())
            {
                var line = vis.Clone();
                line.MoveEndpointByRange(TextPatternRangeEndpoint.End, line, TextPatternRangeEndpoint.Start);
                for (int n = 0; n < 2000 && chars < maxChars; n++)
                {
                    line.ExpandToEnclosingUnit(TextUnit.Line);
                    if (line.CompareEndpoints(TextPatternRangeEndpoint.Start, vis, TextPatternRangeEndpoint.End) >= 0) break;
                    string t = line.GetText(Math.Max(1, maxChars - chars)).TrimEnd('\r', '\n');
                    if (t.Trim().Length > 0)
                    {
                        int[] rect = Union(line.GetBoundingRectangles());
                        if (clip == null || Intersects(rect, clip))
                        {
                            spans.Add(Span(t, rect, r, "uia"));
                            chars += t.Length + 1;
                            got++;
                        }
                    }
                    if (line.Move(TextUnit.Line, 1) == 0) break;
                }
            }
            got += MissedLines(el, tp, r, clip, spans, ref chars, maxChars);
            return got > 0;
        }

        // Some providers (classic Win32 edits through the MSAA proxy) report
        // stale visible ranges, so a line typed a moment ago is missing from
        // the walk above. The whole document is compared against it, and a
        // missing line is added with a rectangle estimated from the lines
        // that were found, as long as it falls inside the control.
        static int MissedLines(AutomationElement el, TextPattern tp, int r, int[] clip, List<Dictionary<string, object>> spans, ref int chars, int maxChars)
        {
            string doc;
            try { doc = tp.DocumentRange.GetText(20001); }
            catch (Exception) { return 0; }
            if (doc == null || doc.Length > 20000) return 0;
            int[] box = ToRect(Get(el, AutomationElement.BoundingRectangleProperty, false));
            if (box == null) return 0;
            string want = r > 0 ? RefStr(r) : null;
            var have = new HashSet<string>();
            int[] first = null;
            string firstText = null;
            foreach (var s in spans)
            {
                if ((s["ref"] as string) != want) continue;
                have.Add(((string)s["text"]).Trim());
                if (first == null) { first = s["rect"] as int[]; firstText = ((string)s["text"]).Trim(); }
            }
            string[] lines = doc.Replace("\r\n", "\n").Replace('\r', '\n').Split('\n');
            int lineH = first != null && first[3] > 0 ? first[3] : 16;
            int firstIdx = 0;
            if (firstText != null)
                for (int k = 0; k < lines.Length; k++)
                    if (lines[k].Trim() == firstText) { firstIdx = k; break; }
            int top = first != null ? first[1] - firstIdx * lineH : box[1];
            int added = 0;
            for (int i = 0; i < lines.Length && chars < maxChars; i++)
            {
                string t = lines[i].TrimEnd();
                if (t.Trim().Length == 0 || have.Contains(t.Trim())) continue;
                int y = top + i * lineH;
                if (y < box[1] || y + lineH > box[1] + box[3]) continue;
                int[] rect = new int[] { box[0] + 4, y, Math.Max(8, box[2] - 8), lineH };
                if (clip != null && !Intersects(rect, clip)) continue;
                spans.Add(Span(t, rect, r, "uia"));
                chars += t.Length + 1;
                added++;
            }
            return added;
        }

        static object GetText(Dictionary<string, object> a)
        {
            int maxChars = Math.Max(100, Int(a, "maxChars", 50000));
            int[] clip = RectArg(a, "rect");
            var spans = new List<Dictionary<string, object>>();
            int chars = 0;
            var sw = Stopwatch.StartNew();
            a["depth"] = Int(a, "depth", 60);
            Walk w = null;
            int gen;
            w = RunWalk(a, "visible", delegate (AutomationElement el, Info i, int r, string parentName)
            {
                if (chars >= maxChars) { w.Truncated = true; return false; }
                if (!TextRoles.Contains(i.ControlTypeId) || i.Password) return true;
                if (i.Text && (i.ControlTypeId == ControlType.Document.Id || i.ControlTypeId == ControlType.Edit.Id))
                {
                    // The document's own text covers its children.
                    try { if (LineSpans(el, r, clip, spans, ref chars, maxChars)) return false; }
                    catch (Exception) { }
                }
                string text = !string.IsNullOrEmpty(i.Value) ? i.Value : (i.Name != parentName ? i.Name : "");
                if (text.Trim().Length == 0) return true;
                if (clip != null && !Intersects(i.Rect, clip)) return true;
                text = Clip(text, Math.Max(1, maxChars - chars));
                spans.Add(Span(text, i.Rect, r, "uia"));
                chars += text.Length + 1;
                return true;
            }, out gen);
            // Reading order: top then left, with spans on one visual line
            // (within half a line height) kept left to right. Spans of one
            // element keep their own order.
            var ordered = new List<KeyValuePair<int, Dictionary<string, object>>>();
            for (int n = 0; n < spans.Count; n++) ordered.Add(new KeyValuePair<int, Dictionary<string, object>>(n, spans[n]));
            ordered.Sort(delegate (KeyValuePair<int, Dictionary<string, object>> x, KeyValuePair<int, Dictionary<string, object>> y)
            {
                var rx = x.Value["rect"] as int[];
                var ry = y.Value["rect"] as int[];
                if (rx == null || ry == null) return x.Key.CompareTo(y.Key);
                int bx = (rx[1] + rx[3] / 2) / 12, by = (ry[1] + ry[3] / 2) / 12;
                if (bx != by) return bx.CompareTo(by);
                if (rx[0] != ry[0]) return rx[0].CompareTo(ry[0]);
                return x.Key.CompareTo(y.Key);
            });
            var outSpans = new List<object>();
            var sb = new StringBuilder();
            foreach (var kv in ordered) { outSpans.Add(kv.Value); sb.Append((string)kv.Value["text"]).Append('\n'); }
            var res = new Dictionary<string, object>();
            res["windowHandle"] = HandleStr(w.Top);
            res["windowRect"] = w.WinRect;
            res["text"] = sb.ToString();
            res["spans"] = outSpans;
            res["source"] = "uia";
            res["truncated"] = w.Truncated;
            res["readMs"] = sw.ElapsedMilliseconds;
            return res;
        }

        static object At(Dictionary<string, object> a)
        {
            int x = RequireInt(a, "x"), y = RequireInt(a, "y");
            IntPtr hit = Native.WindowFromPoint(new Native.POINT { X = x, Y = y });
            IntPtr top = Native.GetAncestor(hit, Native.GA_ROOT);
            if (IsExcludedWindow(hit)) throw new DesktopError("BLOCKED_OWN_WINDOW", "that point is on AevalBuddy's own window");
            AutomationElement el;
            var cr = MakeCache(TreeScope.Element);
            using (cr.Activate()) { el = AutomationElement.FromPoint(new System.Windows.Point(x, y)); }
            Info i = Read(el, true);
            if (IsExcludedPid(i.Pid)) throw new DesktopError("BLOCKED_OWN_WINDOW", "that point is on AevalBuddy's own window");
            var res = new Dictionary<string, object>();
            res["node"] = NodeDict(i, RefFor(el, i, top), 0, 0, 300);
            res["windowHandle"] = HandleStr(top);
            return res;
        }

        // The element with keyboard focus, so the agent's gate can see what a
        // Space or Enter would press. node is null when the window asked for
        // is not in front: keys sent to it land on its own remembered focus,
        // which UI Automation cannot see from outside.
        static object FocusedCmd(Dictionary<string, object> a)
        {
            IntPtr want = new IntPtr(ParseHandle(Arg(a, "windowHandle")));
            if (want != IntPtr.Zero) want = Native.GetAncestor(want, Native.GA_ROOT);
            IntPtr fg = ForegroundRoot();
            var res = new Dictionary<string, object>();
            res["node"] = null;
            res["windowHandle"] = HandleStr(fg);
            if (fg == IntPtr.Zero || IsExcludedWindow(fg) || (want != IntPtr.Zero && want != fg)) return res;
            AutomationElement el;
            var cr = MakeCache(TreeScope.Element);
            using (cr.Activate()) { el = AutomationElement.FocusedElement; }
            if (el == null) return res;
            Info i = Read(el, true);
            if (IsExcludedPid(i.Pid)) return res;
            res["node"] = NodeDict(i, RefFor(el, i, fg), 0, 0, 300);
            return res;
        }

        static object Resolve(Dictionary<string, object> a)
        {
            var e = GetRef(Arg(a, "ref"));
            Info i = Read(e.El, false);
            var res = new Dictionary<string, object>();
            res["node"] = NodeDict(i, RefNum(Arg(a, "ref")), 0, 0, Int(a, "valueMax", 4000));
            res["windowHandle"] = HandleStr(e.Top);
            return res;
        }

        // ------------------------------------------------------------ pixels

        static int[] CaptureRect(Dictionary<string, object> a)
        {
            int[] rect = RectArg(a, "rect");
            if (rect == null)
            {
                IntPtr h = TargetWindow(a);
                rect = WindowRect(h);
            }
            if (rect == null || rect[2] <= 0 || rect[3] <= 0) throw new DesktopError("BAD_ARGS", "nothing to capture: empty rect");
            return rect;
        }

        // GDI capture of the screen as composed. Our own windows carry
        // WDA_EXCLUDEFROMCAPTURE, so they are already missing from it.
        static Bitmap Grab(int[] r)
        {
            var bmp = new Bitmap(r[2], r[3], PixelFormat.Format32bppArgb);
            using (var g = Graphics.FromImage(bmp))
                g.CopyFromScreen(r[0], r[1], 0, 0, new Size(r[2], r[3]), CopyPixelOperation.SourceCopy);
            return bmp;
        }

        static Bitmap Scale(Bitmap src, double s)
        {
            int w = Math.Max(1, (int)Math.Round(src.Width * s)), h = Math.Max(1, (int)Math.Round(src.Height * s));
            var dst = new Bitmap(w, h, PixelFormat.Format32bppArgb);
            using (var g = Graphics.FromImage(dst))
            {
                g.InterpolationMode = s < 1 ? InterpolationMode.HighQualityBicubic : InterpolationMode.Bicubic;
                g.PixelOffsetMode = PixelOffsetMode.HighQuality;
                g.DrawImage(src, 0, 0, w, h);
            }
            return dst;
        }

        // screenshot keeps a big capture under maxEdge; zoom also enlarges a
        // small region up to it, for text too small to read at 1:1.
        static object CaptureCmd(Dictionary<string, object> a)
        {
            int[] r = CaptureRect(a);
            int maxEdge = Math.Max(64, Int(a, "maxEdge", 1568));
            bool enlarge = Bool(a, "enlarge", false);
            double s = (double)maxEdge / Math.Max(r[2], r[3]);
            if (!enlarge) s = Math.Min(1, s);
            else s = Math.Min(s, 4);
            using (var raw = Grab(r))
            {
                Bitmap img = Math.Abs(s - 1) < 0.001 ? raw : Scale(raw, s);
                try
                {
                    using (var ms = new MemoryStream())
                    {
                        img.Save(ms, ImageFormat.Png);
                        var res = new Dictionary<string, object>();
                        res["dataUrl"] = "data:image/png;base64," + Convert.ToBase64String(ms.ToArray());
                        res["rect"] = r;
                        res["scale"] = s;
                        res["width"] = img.Width;
                        res["height"] = img.Height;
                        return res;
                    }
                }
                finally { if (!ReferenceEquals(img, raw)) img.Dispose(); }
            }
        }

        static object OcrCmd(Dictionary<string, object> a)
        {
            int[] r = CaptureRect(a);
            var sw = Stopwatch.StartNew();
            using (var raw = Grab(r))
            {
                var res = Ocr.Recognize(raw, r, Str(a, "lang", "auto"));
                res["rect"] = r;
                res["ocrMs"] = sw.ElapsedMilliseconds;
                return res;
            }
        }

        // ------------------------------------------------------------ input

        static uint SendInputs(Native.INPUT[] inputs)
        {
            return Native.SendInput((uint)inputs.Length, inputs, Marshal.SizeOf(typeof(Native.INPUT)));
        }

        static Native.INPUT MouseInput(int dx, int dy, uint flags, int data)
        {
            var inp = new Native.INPUT();
            inp.type = Native.INPUT_MOUSE;
            inp.u.mi.dx = dx;
            inp.u.mi.dy = dy;
            inp.u.mi.mouseData = unchecked((uint)data);
            inp.u.mi.dwFlags = flags;
            inp.u.mi.dwExtraInfo = Native.EXTRA;
            return inp;
        }

        static void SendMouseRel(int dx, int dy, uint flags, int data)
        {
            SendInputs(new Native.INPUT[] { MouseInput(dx, dy, flags, data) });
        }

        // MOUSEEVENTF_ABSOLUTE | VIRTUALDESK takes 0..65535 across the whole
        // virtual screen. Rounding can land a pixel off on wide desktops, so
        // the position is checked and corrected.
        static void SendMouseAbs(int x, int y, uint flags, int data)
        {
            int vx = Native.GetSystemMetrics(76), vy = Native.GetSystemMetrics(77);
            int vw = Math.Max(2, Native.GetSystemMetrics(78)), vh = Math.Max(2, Native.GetSystemMetrics(79));
            int nx = (int)Math.Round((x - vx) * 65535.0 / (vw - 1));
            int ny = (int)Math.Round((y - vy) * 65535.0 / (vh - 1));
            SendInputs(new Native.INPUT[] { MouseInput(nx, ny, Native.MOUSEEVENTF_MOVE | Native.MOUSEEVENTF_ABSOLUTE | Native.MOUSEEVENTF_VIRTUALDESK | flags, data) });
        }

        static void MoveTo(int x, int y)
        {
            SaveCursor();
            SendMouseAbs(x, y, 0, 0);
            Native.POINT p;
            if (Native.GetCursorPos(out p) && (p.X != x || p.Y != y)) Native.SetCursorPos(x, y);
        }

        static void SaveCursor()
        {
            lock (cursorLock)
            {
                if (savedCursor.HasValue) return;
                Native.POINT p;
                if (Native.GetCursorPos(out p)) savedCursor = p;
            }
        }

        static object RestoreCursor()
        {
            Native.POINT? p;
            lock (cursorLock) { p = savedCursor; savedCursor = null; }
            var res = new Dictionary<string, object>();
            res["ok"] = p.HasValue;
            if (p.HasValue)
            {
                Native.SetCursorPos(p.Value.X, p.Value.Y);
                res["point"] = new int[] { p.Value.X, p.Value.Y };
            }
            return res;
        }

        static readonly HashSet<ushort> ExtendedKeys = new HashSet<ushort> {
            0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2D, 0x2E, 0x5B, 0x5C, 0x5D, 0x6F, 0x90, 0x2C,
        };

        static Native.INPUT KeyInput(ushort vk, bool up)
        {
            var k = new Native.INPUT();
            k.type = Native.INPUT_KEYBOARD;
            k.u.ki.wVk = vk;
            k.u.ki.wScan = (ushort)Native.MapVirtualKey(vk, 0);
            k.u.ki.dwFlags = (up ? Native.KEYEVENTF_KEYUP : 0) | (ExtendedKeys.Contains(vk) ? Native.KEYEVENTF_EXTENDEDKEY : 0);
            k.u.ki.dwExtraInfo = Native.EXTRA;
            return k;
        }

        static Native.INPUT UnicodeInput(char c, bool up)
        {
            var k = new Native.INPUT();
            k.type = Native.INPUT_KEYBOARD;
            k.u.ki.wVk = 0;
            k.u.ki.wScan = c;
            k.u.ki.dwFlags = Native.KEYEVENTF_UNICODE | (up ? Native.KEYEVENTF_KEYUP : 0);
            k.u.ki.dwExtraInfo = Native.EXTRA;
            return k;
        }

        static readonly Dictionary<string, ushort> KeyNames = BuildKeyNames();

        static Dictionary<string, ushort> BuildKeyNames()
        {
            var m = new Dictionary<string, ushort>();
            m["ctrl"] = 0x11; m["control"] = 0x11; m["shift"] = 0x10; m["alt"] = 0x12; m["option"] = 0x12;
            m["win"] = 0x5B; m["meta"] = 0x5B; m["super"] = 0x5B; m["cmd"] = 0x5B; m["windows"] = 0x5B;
            m["enter"] = 0x0D; m["return"] = 0x0D; m["tab"] = 0x09; m["esc"] = 0x1B; m["escape"] = 0x1B;
            m["space"] = 0x20; m["backspace"] = 0x08; m["back"] = 0x08; m["delete"] = 0x2E; m["del"] = 0x2E;
            m["insert"] = 0x2D; m["ins"] = 0x2D; m["home"] = 0x24; m["end"] = 0x23;
            m["pageup"] = 0x21; m["pgup"] = 0x21; m["page_up"] = 0x21; m["prior"] = 0x21;
            m["pagedown"] = 0x22; m["pgdn"] = 0x22; m["page_down"] = 0x22; m["next"] = 0x22;
            m["left"] = 0x25; m["up"] = 0x26; m["right"] = 0x27; m["down"] = 0x28;
            m["arrowleft"] = 0x25; m["arrowup"] = 0x26; m["arrowright"] = 0x27; m["arrowdown"] = 0x28;
            m["capslock"] = 0x14; m["printscreen"] = 0x2C; m["print"] = 0x2C; m["menu"] = 0x5D; m["apps"] = 0x5D;
            m["numlock"] = 0x90; m["scrolllock"] = 0x91; m["pause"] = 0x13;
            m["plus"] = 0xBB; m["equal"] = 0xBB; m["minus"] = 0xBD; m["comma"] = 0xBC; m["period"] = 0xBE;
            m["add"] = 0x6B; m["subtract"] = 0x6D; m["multiply"] = 0x6A; m["divide"] = 0x6F; m["decimal"] = 0x6E;
            for (int i = 1; i <= 24; i++) m["f" + i] = (ushort)(0x70 + i - 1);
            for (int i = 0; i <= 9; i++) m["numpad" + i] = (ushort)(0x60 + i);
            return m;
        }

        static readonly HashSet<ushort> Modifiers = new HashSet<ushort> { 0x11, 0x10, 0x12, 0x5B };

        // "ctrl+shift+s" -> [ctrl, shift, s]. A single character that is not
        // a letter or digit goes through VkKeyScan, which also says whether
        // the current layout needs shift for it.
        static List<ushort> ParseChord(string chord)
        {
            var keys = new List<ushort>();
            var parts = chord.Split('+');
            for (int n = 0; n < parts.Length; n++)
            {
                string p = parts[n];
                if (p.Length == 0 && n == parts.Length - 1 && n > 0) p = "+";
                if (p.Length == 0) continue;
                string lower = p.ToLowerInvariant();
                ushort vk;
                if (KeyNames.TryGetValue(lower, out vk)) { keys.Add(vk); continue; }
                if (p.Length == 1)
                {
                    char c = p[0];
                    if (c >= 'a' && c <= 'z') { keys.Add((ushort)(c - 'a' + 0x41)); continue; }
                    if (c >= 'A' && c <= 'Z') { keys.Add((ushort)(c - 'A' + 0x41)); continue; }
                    if (c >= '0' && c <= '9') { keys.Add((ushort)c); continue; }
                    short scan = Native.VkKeyScan(c);
                    if (scan != -1)
                    {
                        if ((scan & 0x100) != 0 && !keys.Contains(0x10)) keys.Insert(0, 0x10);
                        keys.Add((ushort)(scan & 0xFF));
                        continue;
                    }
                }
                throw new DesktopError("BAD_ARGS", "unknown key '" + p + "' in '" + chord + "'");
            }
            if (keys.Count == 0) throw new DesktopError("BAD_ARGS", "empty key chord");
            return keys;
        }

        static void SendChord(List<ushort> keys)
        {
            var list = new List<Native.INPUT>();
            foreach (var k in keys) list.Add(KeyInput(k, false));
            for (int n = keys.Count - 1; n >= 0; n--) list.Add(KeyInput(keys[n], true));
            SendInputs(list.ToArray());
        }

        static void HoldModifiers(List<ushort> mods, bool up)
        {
            if (mods == null || mods.Count == 0) return;
            var list = new List<Native.INPUT>();
            if (!up) foreach (var k in mods) list.Add(KeyInput(k, false));
            else for (int n = mods.Count - 1; n >= 0; n--) list.Add(KeyInput(mods[n], true));
            SendInputs(list.ToArray());
        }

        static List<ushort> ParseMods(Dictionary<string, object> a)
        {
            string m = Str(a, "modifiers", "");
            if (m.Length == 0) return null;
            var keys = ParseChord(m);
            foreach (var k in keys) if (!Modifiers.Contains(k)) throw new DesktopError("BAD_ARGS", "modifiers may only hold ctrl, shift, alt, win");
            return keys;
        }

        // Keyboard input goes wherever focus is, so it is only sent while the
        // intended window is in front. With a window that window is brought
        // forward first; without one, the foreground window is the target
        // unless it is AevalBuddy.
        // A locked session (LockApp in front) or a UAC prompt (the input
        // desktop is not ours) takes no input from us, and must not: typing
        // at a lock screen is typing into a password box.
        static void RefuseSecureDesktop()
        {
            IntPtr fg = Native.GetForegroundWindow();
            if (fg != IntPtr.Zero && string.Equals(ProcessName(PidOf(fg), null), "LockApp", StringComparison.OrdinalIgnoreCase))
                throw new DesktopError("SECURE_DESKTOP", "the session is locked; nothing can be clicked or typed until it is unlocked");
            IntPtr desk = Native.OpenInputDesktop(0, false, 0x0100);
            if (desk == IntPtr.Zero)
                throw new DesktopError("SECURE_DESKTOP", "a secure desktop (sign-in or UAC prompt) has the input; it cannot be driven");
            Native.CloseDesktop(desk);
        }

        // An element can be the right target and still refuse SetFocus: a
        // classic Win32 edit exposed through the MSAA proxy reports itself as
        // a Document that "cannot receive focus" while the HWND underneath
        // takes the keyboard fine. So focus goes to the element if it takes
        // it, else to its nearest focusable descendant, else to a focusable
        // ancestor short of the window, else nowhere: the window is already
        // in front and its own focus is where keystrokes land.
        static AutomationElement FocusableNear(AutomationElement el)
        {
            if (IsFocusable(el)) return el;
            var walker = TreeWalker.ControlViewWalker;
            var queue = new Queue<AutomationElement>();
            queue.Enqueue(el);
            int seen = 0;
            while (queue.Count > 0 && seen < 200)
            {
                var cur = queue.Dequeue();
                AutomationElement c = null;
                try { c = walker.GetFirstChild(cur); } catch (Exception) { c = null; }
                while (c != null && seen < 200)
                {
                    seen++;
                    if (IsFocusable(c)) return c;
                    queue.Enqueue(c);
                    try { c = walker.GetNextSibling(c); } catch (Exception) { c = null; }
                }
            }
            AutomationElement p = null;
            try { p = walker.GetParent(el); } catch (Exception) { p = null; }
            for (int n = 0; p != null && n < 4; n++)
            {
                try
                {
                    if (p.Current.ControlType == ControlType.Window) break;
                }
                catch (Exception) { break; }
                if (IsFocusable(p)) return p;
                try { p = walker.GetParent(p); } catch (Exception) { p = null; }
            }
            return null;
        }

        static bool IsFocusable(AutomationElement el)
        {
            try
            {
                object v = el.GetCurrentPropertyValue(AutomationElement.IsKeyboardFocusableProperty, false);
                return v is bool && (bool)v;
            }
            catch (Exception) { return false; }
        }

        static string FocusElement(AutomationElement el)
        {
            var f = FocusableNear(el);
            if (f == null) return "window";
            try { f.SetFocus(); }
            catch (InvalidOperationException) { return "window"; }
            return f == el ? "element" : "near";
        }

        static IntPtr KeyboardTarget(IntPtr target)
        {
            RefuseSecureDesktop();
            if (target != IntPtr.Zero)
            {
                if (!Native.IsWindow(target)) throw new DesktopError("NOT_FOUND", "that window no longer exists");
                target = Native.GetAncestor(target, Native.GA_ROOT);
                RefuseExcluded(target);
                RefuseElevated(target);
                if (ForegroundRoot() != target)
                {
                    if (FocusWindow(target) == null)
                        throw new DesktopError("NOT_FOREGROUND", "\"" + Title(target) + "\" could not be brought to the front");
                    Thread.Sleep(60);
                }
                return target;
            }
            IntPtr fg = ForegroundRoot();
            if (fg == IntPtr.Zero) throw new DesktopError("NOT_FOUND", "no foreground window");
            if (IsExcludedWindow(fg)) throw new DesktopError("BLOCKED_OWN_WINDOW", "the foreground window is AevalBuddy's own; focus the target first");
            RefuseElevated(fg);
            return fg;
        }

        static void GuardStill(IntPtr target, string what, int done)
        {
            if (ForegroundRoot() != target)
                throw new DesktopError("FOCUS_LOST", "the target window lost focus after " + done + " " + what + "; stopped so nothing lands in the wrong app");
        }

        static int[] Center(int[] r)
        {
            return new int[] { r[0] + r[2] / 2, r[1] + r[3] / 2 };
        }

        static int[] PointFor(RefEntry e, out Info info)
        {
            info = Read(e.El, false);
            if (info.Offscreen && info.ScrollItem)
            {
                try
                {
                    ((ScrollItemPattern)e.El.GetCurrentPattern(ScrollItemPattern.Pattern)).ScrollIntoView();
                    Thread.Sleep(150);
                    info = Read(e.El, false);
                }
                catch (InvalidOperationException) { }
            }
            if (info.Rect != null && info.Rect[2] > 0 && info.Rect[3] > 0) return Center(info.Rect);
            System.Windows.Point p;
            if (e.El.TryGetClickablePoint(out p)) return new int[] { (int)Math.Round(p.X), (int)Math.Round(p.Y) };
            throw new DesktopError("NOT_FOUND", "that element has no on-screen rectangle");
        }

        // What is actually under (x, y). With an expected window, anything
        // else on top (a toast, another app) is reported instead of clicked.
        static IntPtr CheckPoint(int x, int y, RefEntry expect, bool force)
        {
            RefuseOwnPoint(x, y);
            RefuseSecureDesktop();
            IntPtr hit = Native.WindowFromPoint(new Native.POINT { X = x, Y = y });
            IntPtr hitTop = Native.GetAncestor(hit, Native.GA_ROOT);
            if (expect != null && !force && hitTop != expect.Top && PidOf(hitTop) != (uint)expect.Pid && PidOf(hitTop) != PidOf(expect.Top))
                throw new DesktopError("OCCLUDED", "another window (\"" + Title(hitTop) + "\") covers that element; pass force to click anyway");
            RefuseElevated(hitTop);
            return hitTop;
        }

        // A target is {ref} or {x, y}.
        static int[] TargetPoint(Dictionary<string, object> a, out RefEntry entry, out Info info)
        {
            entry = null;
            info = null;
            if (Arg(a, "ref") != null)
            {
                entry = GetRef(Arg(a, "ref"));
                RefuseElevated(entry.Top);
                return PointFor(entry, out info);
            }
            return new int[] { RequireInt(a, "x"), RequireInt(a, "y") };
        }

        // Invoke on a Win32 button that opens a modal dialog does not return
        // until the dialog closes. The call keeps running on its own thread
        // and the reply says it is still pending instead of timing out.
        static bool RunBounded(Action act, int ms)
        {
            Exception err = null;
            var t = new Thread(delegate () { ThreadDpi(); try { act(); } catch (Exception e) { err = e; } });
            t.IsBackground = true;
            t.SetApartmentState(ApartmentState.MTA);
            t.Start();
            if (!t.Join(ms)) return false;
            if (err != null) throw err;
            return true;
        }

        static object Click(Dictionary<string, object> a)
        {
            string button = Str(a, "button", "left");
            int count = Math.Max(1, Math.Min(3, Int(a, "count", 1)));
            var mods = ParseMods(a);
            bool force = Bool(a, "force", false);
            RefEntry e;
            Info info;
            int[] p = TargetPoint(a, out e, out info);
            var res = new Dictionary<string, object>();
            res["point"] = p;
            // Pattern first: a plain left click on something invokable does
            // not need the person's cursor at all.
            if (e != null && info != null && info.Invoke && button == "left" && count == 1 && mods == null && !Bool(a, "physical", false))
            {
                if (!info.Enabled) throw new DesktopError("DISABLED", "that element is disabled");
                var el = e.El;
                bool done = RunBounded(delegate () { ((InvokePattern)el.GetCurrentPattern(InvokePattern.Pattern)).Invoke(); }, 2000);
                res["ok"] = true;
                res["via"] = "invoke";
                res["pending"] = !done;
                return res;
            }
            uint down, up;
            if (button == "left") { down = 0x2; up = 0x4; }
            else if (button == "right") { down = 0x8; up = 0x10; }
            else if (button == "middle") { down = 0x20; up = 0x40; }
            else throw new DesktopError("BAD_ARGS", "button must be left, right or middle");
            IntPtr hitTop = CheckPoint(p[0], p[1], e, force);
            MoveTo(p[0], p[1]);
            Thread.Sleep(20);
            HoldModifiers(mods, false);
            try
            {
                for (int n = 0; n < count; n++)
                {
                    SendMouseAbs(p[0], p[1], down, 0);
                    SendMouseAbs(p[0], p[1], up, 0);
                    if (n + 1 < count) Thread.Sleep(40);
                }
            }
            finally { HoldModifiers(mods, true); }
            res["ok"] = true;
            res["via"] = "sendinput";
            res["windowHandle"] = HandleStr(hitTop);
            return res;
        }

        static object Hover(Dictionary<string, object> a)
        {
            RefEntry e;
            Info info;
            int[] p = TargetPoint(a, out e, out info);
            CheckPoint(p[0], p[1], e, Bool(a, "force", false));
            MoveTo(p[0], p[1]);
            var res = new Dictionary<string, object>();
            res["ok"] = true;
            res["point"] = p;
            return res;
        }

        static int[] PointArg(Dictionary<string, object> a, string k)
        {
            var d = Arg(a, k) as Dictionary<string, object>;
            if (d == null) throw new DesktopError("BAD_ARGS", "missing " + k + " {x, y}");
            return new int[] { RequireInt(d, "x"), RequireInt(d, "y") };
        }

        static object Drag(Dictionary<string, object> a)
        {
            int[] from = PointArg(a, "from"), to = PointArg(a, "to");
            CheckPoint(from[0], from[1], null, false);
            CheckPoint(to[0], to[1], null, false);
            int steps = Math.Max(2, Math.Min(60, Int(a, "steps", 12)));
            MoveTo(from[0], from[1]);
            Thread.Sleep(30);
            SendMouseAbs(from[0], from[1], 0x2, 0);
            try
            {
                for (int n = 1; n <= steps; n++)
                {
                    int x = from[0] + (to[0] - from[0]) * n / steps, y = from[1] + (to[1] - from[1]) * n / steps;
                    SendMouseAbs(x, y, 0, 0);
                    Thread.Sleep(12);
                }
            }
            finally { SendMouseAbs(to[0], to[1], 0x4, 0); }
            var res = new Dictionary<string, object>();
            res["ok"] = true;
            res["from"] = from;
            res["to"] = to;
            return res;
        }

        static object TypeCmd(Dictionary<string, object> a)
        {
            string text = Str(a, "text", null);
            if (text == null) throw new DesktopError("BAD_ARGS", "missing text");
            IntPtr win = new IntPtr(ParseHandle(Arg(a, "windowHandle")));
            RefEntry e = null;
            if (Arg(a, "ref") != null)
            {
                e = GetRef(Arg(a, "ref"));
                win = e.Top;
            }
            IntPtr target = KeyboardTarget(win);
            string focusedVia = "window";
            if (e != null)
            {
                focusedVia = FocusElement(e.El);
                Thread.Sleep(40);
            }
            TypeInto(target, text);
            var r = new Dictionary<string, object>();
            r["ok"] = true;
            r["typed"] = text.Length;
            r["focus"] = focusedVia;
            r["windowHandle"] = HandleStr(target);
            return r;
        }

        static void TypeInto(IntPtr target, string text)
        {
            const int chunk = 24;
            text = text.Replace("\r\n", "\n");
            int start = 0;
            while (start < text.Length)
            {
                GuardStill(target, "characters", start);
                int end = Math.Min(text.Length, start + chunk);
                // Never split a surrogate pair across chunks.
                if (end < text.Length && char.IsHighSurrogate(text[end - 1])) end++;
                var list = new List<Native.INPUT>();
                for (int n = start; n < end; n++)
                {
                    char c = text[n];
                    if (c == '\n') { list.Add(KeyInput(0x0D, false)); list.Add(KeyInput(0x0D, true)); }
                    else if (c == '\t') { list.Add(KeyInput(0x09, false)); list.Add(KeyInput(0x09, true)); }
                    else { list.Add(UnicodeInput(c, false)); list.Add(UnicodeInput(c, true)); }
                }
                SendInputs(list.ToArray());
                start = end;
                Thread.Sleep(8);
            }
        }

        static object KeyCmd(Dictionary<string, object> a)
        {
            string keys = Str(a, "keys", "").Trim();
            if (keys.Length == 0) throw new DesktopError("BAD_ARGS", "missing keys");
            int repeat = Math.Max(1, Math.Min(100, Int(a, "repeat", 1)));
            var chords = new List<List<ushort>>();
            foreach (var c in keys.Split(new char[] { ' ' }, StringSplitOptions.RemoveEmptyEntries)) chords.Add(ParseChord(c));
            IntPtr target = KeyboardTarget(new IntPtr(ParseHandle(Arg(a, "windowHandle"))));
            int done = 0;
            for (int n = 0; n < repeat; n++)
                foreach (var c in chords)
                {
                    GuardStill(target, "keys", done);
                    SendChord(c);
                    done++;
                    Thread.Sleep(15);
                }
            var r = new Dictionary<string, object>();
            r["ok"] = true;
            r["sent"] = done;
            r["windowHandle"] = HandleStr(target);
            return r;
        }

        static object ScrollCmd(Dictionary<string, object> a)
        {
            string dir = Str(a, "direction", "down").ToLowerInvariant();
            int amount = Math.Max(1, Math.Min(10, Int(a, "amount", 3)));
            int dy = dir == "down" ? amount : dir == "up" ? -amount : 0;
            int dx = dir == "right" ? amount : dir == "left" ? -amount : 0;
            if (dx == 0 && dy == 0) throw new DesktopError("BAD_ARGS", "direction must be up, down, left or right");
            var res = new Dictionary<string, object>();
            RefEntry e;
            Info info;
            int[] p = TargetPoint(a, out e, out info);
            if (e != null && info != null && info.Scroll && !Bool(a, "physical", false))
            {
                var sp = (ScrollPattern)e.El.GetCurrentPattern(ScrollPattern.Pattern);
                for (int n = 0; n < Math.Abs(dy); n++)
                    sp.Scroll(ScrollAmount.NoAmount, dy > 0 ? ScrollAmount.SmallIncrement : ScrollAmount.SmallDecrement);
                for (int n = 0; n < Math.Abs(dx); n++)
                    sp.Scroll(dx > 0 ? ScrollAmount.SmallIncrement : ScrollAmount.SmallDecrement, ScrollAmount.NoAmount);
                res["ok"] = true;
                res["via"] = "pattern";
                return res;
            }
            CheckPoint(p[0], p[1], e, Bool(a, "force", false));
            MoveTo(p[0], p[1]);
            Thread.Sleep(15);
            // Positive dy scrolls down (the wheel turning towards you).
            for (int n = 0; n < Math.Abs(dy); n++) { SendMouseRel(0, 0, Native.MOUSEEVENTF_WHEEL, dy > 0 ? -120 : 120); Thread.Sleep(10); }
            for (int n = 0; n < Math.Abs(dx); n++) { SendMouseRel(0, 0, Native.MOUSEEVENTF_HWHEEL, dx > 0 ? 120 : -120); Thread.Sleep(10); }
            res["ok"] = true;
            res["via"] = "sendinput";
            res["point"] = p;
            return res;
        }

        static object ScrollTo(Dictionary<string, object> a)
        {
            var e = GetRef(Arg(a, "ref"));
            RefuseElevated(e.Top);
            object pat;
            if (!e.El.TryGetCurrentPattern(ScrollItemPattern.Pattern, out pat))
                throw new DesktopError("NO_PATTERN", "that element cannot scroll itself into view (no ScrollItemPattern)");
            ((ScrollItemPattern)pat).ScrollIntoView();
            Thread.Sleep(60);
            var res = new Dictionary<string, object>();
            res["ok"] = true;
            res["node"] = NodeDict(Read(e.El, false), RefNum(Arg(a, "ref")), 0, 0, 300);
            return res;
        }

        static object InvokeCmd(Dictionary<string, object> a)
        {
            var e = GetRef(Arg(a, "ref"));
            RefuseElevated(e.Top);
            string action = Str(a, "action", "invoke");
            var el = e.El;
            Action act;
            switch (action)
            {
                case "invoke": act = delegate () { ((InvokePattern)el.GetCurrentPattern(InvokePattern.Pattern)).Invoke(); }; break;
                case "toggle": act = delegate () { ((TogglePattern)el.GetCurrentPattern(TogglePattern.Pattern)).Toggle(); }; break;
                case "select": act = delegate () { ((SelectionItemPattern)el.GetCurrentPattern(SelectionItemPattern.Pattern)).Select(); }; break;
                case "expand": act = delegate () { ((ExpandCollapsePattern)el.GetCurrentPattern(ExpandCollapsePattern.Pattern)).Expand(); }; break;
                case "collapse": act = delegate () { ((ExpandCollapsePattern)el.GetCurrentPattern(ExpandCollapsePattern.Pattern)).Collapse(); }; break;
                case "focus": act = delegate () { el.SetFocus(); }; break;
                default: throw new DesktopError("BAD_ARGS", "unknown action: " + action);
            }
            bool done;
            try { done = RunBounded(act, 2000); }
            catch (InvalidOperationException ex) { throw new DesktopError("NO_PATTERN", action + " is not supported by that element (" + ex.Message + ")"); }
            var res = new Dictionary<string, object>();
            res["ok"] = true;
            res["via"] = action;
            res["pending"] = !done;
            return res;
        }

        // form_input: a string, a number, a boolean for a checkbox, or an
        // option's text for a combo box, without moving the cursor when a
        // pattern can do it.
        static object SetValueCmd(Dictionary<string, object> a)
        {
            var e = GetRef(Arg(a, "ref"));
            RefuseElevated(e.Top);
            object value = Arg(a, "value");
            if (value == null) throw new DesktopError("BAD_ARGS", "missing value");
            string sval = Convert.ToString(value, System.Globalization.CultureInfo.InvariantCulture);
            Info i = Read(e.El, false);
            if (!i.Enabled) throw new DesktopError("DISABLED", "that element is disabled");
            var res = new Dictionary<string, object>();
            var el = e.El;
            if (value is bool && (i.Toggle || i.Select))
            {
                bool want = (bool)value;
                if (i.Toggle)
                {
                    var tp = (TogglePattern)el.GetCurrentPattern(TogglePattern.Pattern);
                    for (int n = 0; n < 3 && (tp.Current.ToggleState == ToggleState.On) != want; n++) tp.Toggle();
                    res["via"] = "toggle";
                }
                else
                {
                    var sp = (SelectionItemPattern)el.GetCurrentPattern(SelectionItemPattern.Pattern);
                    if (want) sp.Select(); else sp.RemoveFromSelection();
                    res["via"] = "select";
                }
            }
            else if (i.ValueP && !i.ValueRO)
            {
                ((ValuePattern)el.GetCurrentPattern(ValuePattern.Pattern)).SetValue(sval);
                res["via"] = "value";
            }
            else if (i.Range && !(value is bool))
            {
                double v;
                if (!double.TryParse(sval, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out v))
                    throw new DesktopError("BAD_ARGS", "that element takes a number");
                ((RangeValuePattern)el.GetCurrentPattern(RangeValuePattern.Pattern)).SetValue(v);
                res["via"] = "rangevalue";
            }
            else if (i.ControlTypeId == ControlType.ComboBox.Id || i.ControlTypeId == ControlType.List.Id)
            {
                SelectOption(el, i, sval);
                res["via"] = "selectionitem";
            }
            else if (i.Focused || FocusableNear(el) != null)
            {
                // No pattern: focus it (or the focusable part of it), select
                // everything, type over it.
                IntPtr target = KeyboardTarget(e.Top);
                FocusElement(el);
                Thread.Sleep(40);
                GuardStill(target, "keys", 0);
                SendChord(ParseChord("ctrl+a"));
                Thread.Sleep(20);
                if (sval.Length == 0) SendChord(ParseChord("delete"));
                else TypeInto(target, sval);
                res["via"] = "sendinput";
            }
            else throw new DesktopError("NO_PATTERN", "that element takes no value: no Value, Toggle, RangeValue or SelectionItem pattern and no keyboard focus");
            res["ok"] = true;
            try { res["node"] = NodeDict(Read(el, false), RefNum(Arg(a, "ref")), 0, 0, 4000); }
            catch (ElementNotAvailableException) { }
            return res;
        }

        static void SelectOption(AutomationElement el, Info i, string text)
        {
            ExpandCollapsePattern ec = null;
            object pat;
            if (i.Expand && el.TryGetCurrentPattern(ExpandCollapsePattern.Pattern, out pat))
            {
                ec = (ExpandCollapsePattern)pat;
                try { ec.Expand(); Thread.Sleep(120); } catch (InvalidOperationException) { ec = null; }
            }
            try
            {
                var items = el.FindAll(TreeScope.Descendants, new PropertyCondition(AutomationElement.IsSelectionItemPatternAvailableProperty, true));
                AutomationElement match = null;
                foreach (AutomationElement it in items)
                {
                    string n = it.Current.Name ?? "";
                    if (string.Equals(n.Trim(), text.Trim(), StringComparison.OrdinalIgnoreCase)) { match = it; break; }
                    if (match == null && n.IndexOf(text, StringComparison.OrdinalIgnoreCase) >= 0) match = it;
                }
                if (match == null) throw new DesktopError("NOT_FOUND", "no option named \"" + text + "\"");
                ((SelectionItemPattern)match.GetCurrentPattern(SelectionItemPattern.Pattern)).Select();
            }
            finally
            {
                if (ec != null) { try { ec.Collapse(); } catch (Exception) { } }
            }
        }
    }

    // Windows.Media.Ocr through reflection: the WinRT types are reachable
    // from .NET Framework without referencing any .winmd at compile time,
    // which keeps Add-Type's command line the same on every machine.
    internal static class Ocr
    {
        static MethodInfo asTask;
        static MethodInfo asRandomAccess;
        static readonly object initLock = new object();

        static Type W(string n) { return Type.GetType(n + ", Windows.Foundation, ContentType=WindowsRuntime", true); }

        static void Init()
        {
            lock (initLock)
            {
                if (asTask != null) return;
                var asm = Assembly.Load("System.Runtime.WindowsRuntime, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089");
                foreach (var m in asm.GetType("System.WindowsRuntimeSystemExtensions").GetMethods())
                    if (m.Name == "AsTask" && m.IsGenericMethod && m.GetParameters().Length == 1 && m.GetParameters()[0].ParameterType.Name == "IAsyncOperation`1") asTask = m;
                asRandomAccess = asm.GetType("System.IO.WindowsRuntimeStreamExtensions").GetMethod("AsRandomAccessStream");
            }
        }

        static object Await(object op, Type t)
        {
            var task = (Task)asTask.MakeGenericMethod(t).Invoke(null, new object[] { op });
            if (!task.Wait(12000)) throw new DesktopError("TIMEOUT", "OCR did not finish");
            return task.GetType().GetProperty("Result").GetValue(task, null);
        }

        static object Prop(object o, string name) { return o.GetType().GetProperty(name).GetValue(o, null); }

        static object Engine(string lang)
        {
            var engT = W("Windows.Media.Ocr.OcrEngine");
            object eng = null;
            if (!string.IsNullOrEmpty(lang) && lang != "auto")
            {
                var language = Activator.CreateInstance(W("Windows.Globalization.Language"), lang);
                eng = engT.GetMethod("TryCreateFromLanguage").Invoke(null, new object[] { language });
            }
            if (eng == null) eng = engT.GetMethod("TryCreateFromUserProfileLanguages").Invoke(null, null);
            if (eng == null)
            {
                // A user-profile language without an OCR pack: take any
                // installed recogniser rather than none.
                var langs = (IEnumerable)engT.GetProperty("AvailableRecognizerLanguages").GetValue(null, null);
                foreach (var l in langs) { eng = engT.GetMethod("TryCreateFromLanguage").Invoke(null, new object[] { l }); if (eng != null) break; }
            }
            if (eng == null) throw new DesktopError("NO_OCR", "no Windows OCR language is installed (Settings > Time & language > Language > add a language with optical character recognition)");
            return eng;
        }

        public static Dictionary<string, object> Recognize(Bitmap src, int[] origin, string lang)
        {
            Init();
            var eng = Engine(lang);
            int maxDim = Convert.ToInt32(W("Windows.Media.Ocr.OcrEngine").GetProperty("MaxImageDimension").GetValue(null, null));
            // Small regions are enlarged: the recogniser misses text under
            // about 10px tall, and doubling costs little at that size.
            double s = 1;
            int longEdge = Math.Max(src.Width, src.Height);
            if (longEdge < 1400) s = 2;
            if (longEdge * s > maxDim) s = (double)maxDim / longEdge;
            Bitmap img = Math.Abs(s - 1) < 0.001 ? src : ScaleFor(src, s);
            try
            {
                using (var ms = new MemoryStream())
                {
                    img.Save(ms, ImageFormat.Bmp);
                    ms.Position = 0;
                    var ras = asRandomAccess.Invoke(null, new object[] { ms });
                    var bdT = W("Windows.Graphics.Imaging.BitmapDecoder");
                    MethodInfo create = null, getBitmap = null;
                    foreach (var m in bdT.GetMethods())
                    {
                        if (m.Name == "CreateAsync" && m.GetParameters().Length == 1) create = m;
                        if (m.Name == "GetSoftwareBitmapAsync" && m.GetParameters().Length == 0) getBitmap = m;
                    }
                    var dec = Await(create.Invoke(null, new object[] { ras }), bdT);
                    var bmp = Await(getBitmap.Invoke(dec, null), W("Windows.Graphics.Imaging.SoftwareBitmap"));
                    var result = Await(eng.GetType().GetMethod("RecognizeAsync").Invoke(eng, new object[] { bmp }), W("Windows.Media.Ocr.OcrResult"));
                    var spans = new List<object>();
                    var sb = new StringBuilder();
                    foreach (var line in (IEnumerable)Prop(result, "Lines"))
                    {
                        bool first = true;
                        foreach (var word in (IEnumerable)Prop(line, "Words"))
                        {
                            var r = Prop(word, "BoundingRect");
                            double x = Convert.ToDouble(Prop(r, "X")), y = Convert.ToDouble(Prop(r, "Y"));
                            double w = Convert.ToDouble(Prop(r, "Width")), h = Convert.ToDouble(Prop(r, "Height"));
                            var d = new Dictionary<string, object>();
                            string t = (string)Prop(word, "Text");
                            d["text"] = t;
                            d["rect"] = new int[] { origin[0] + (int)Math.Round(x / s), origin[1] + (int)Math.Round(y / s), (int)Math.Round(w / s), (int)Math.Round(h / s) };
                            d["ref"] = null;
                            d["source"] = "ocr";
                            spans.Add(d);
                            if (!first) sb.Append(' ');
                            sb.Append(t);
                            first = false;
                        }
                        sb.Append('\n');
                    }
                    var res = new Dictionary<string, object>();
                    res["spans"] = spans;
                    res["text"] = sb.ToString();
                    try { res["lang"] = Convert.ToString(Prop(Prop(eng, "RecognizerLanguage"), "LanguageTag")); } catch (Exception) { }
                    return res;
                }
            }
            finally { if (!ReferenceEquals(img, src)) img.Dispose(); }
        }

        static Bitmap ScaleFor(Bitmap src, double s)
        {
            int w = Math.Max(1, (int)Math.Round(src.Width * s)), h = Math.Max(1, (int)Math.Round(src.Height * s));
            var dst = new Bitmap(w, h, PixelFormat.Format32bppArgb);
            using (var g = Graphics.FromImage(dst))
            {
                g.InterpolationMode = InterpolationMode.HighQualityBicubic;
                g.DrawImage(src, 0, 0, w, h);
            }
            return dst;
        }
    }

    // Physical input watch. Low-level hooks see every mouse and keyboard
    // event with a flag saying whether it was injected; anything without it
    // came from the person, and the agent pauses on it. The hook callback
    // only queues: Windows silently unhooks a callback that stalls.
    // Key identity is never reported, only that a key went down.
    internal static class Watch
    {
        static Thread thread;
        static uint threadId;
        static IntPtr mouseHook, keyHook;
        static Native.HookProc mouseProc, keyProc;
        static readonly object lk = new object();
        static readonly Queue<Dictionary<string, object>> pending = new Queue<Dictionary<string, object>>();
        static readonly AutoResetEvent signal = new AutoResetEvent(false);
        static Thread writer;
        static long lastMove, lastKey, lastButton;
        static int lastX = int.MinValue, lastY;
        static double travelled;
        public static bool On { get { return thread != null; } }

        public static object Set(bool on)
        {
            lock (lk)
            {
                if (on && thread == null)
                {
                    var ready = new ManualResetEvent(false);
                    string error = null;
                    thread = new Thread(delegate ()
                    {
                        threadId = Native.GetCurrentThreadId();
                        Host.ThreadDpi();
                        mouseProc = MouseHook;
                        keyProc = KeyHook;
                        IntPtr mod = Native.GetModuleHandle("user32.dll");
                        mouseHook = Native.SetWindowsHookEx(14, mouseProc, mod, 0);
                        keyHook = Native.SetWindowsHookEx(13, keyProc, mod, 0);
                        if (mouseHook == IntPtr.Zero || keyHook == IntPtr.Zero) error = "SetWindowsHookEx failed: " + Marshal.GetLastWin32Error();
                        ready.Set();
                        Native.MSG msg;
                        while (Native.GetMessage(out msg, IntPtr.Zero, 0, 0) > 0) { }
                        if (mouseHook != IntPtr.Zero) Native.UnhookWindowsHookEx(mouseHook);
                        if (keyHook != IntPtr.Zero) Native.UnhookWindowsHookEx(keyHook);
                        mouseHook = keyHook = IntPtr.Zero;
                    });
                    thread.IsBackground = true;
                    thread.Start();
                    ready.WaitOne(3000);
                    if (writer == null)
                    {
                        writer = new Thread(Drain);
                        writer.IsBackground = true;
                        writer.Start();
                    }
                    if (error != null) { Stop(); throw new DesktopError("INTERNAL", error); }
                }
                else if (!on) Stop();
                var res = new Dictionary<string, object>();
                res["watching"] = thread != null;
                return res;
            }
        }

        public static void Stop()
        {
            lock (lk)
            {
                if (thread == null) return;
                Native.PostThreadMessage(threadId, Native.WM_QUIT, IntPtr.Zero, IntPtr.Zero);
                thread.Join(1000);
                thread = null;
            }
        }

        static void Drain()
        {
            while (true)
            {
                signal.WaitOne();
                while (true)
                {
                    Dictionary<string, object> ev = null;
                    lock (pending) { if (pending.Count > 0) ev = pending.Dequeue(); }
                    if (ev == null) break;
                    try { Host.Write(ev); } catch (Exception) { }
                }
            }
        }

        public static void Emit(string kind, int x, int y, double dist, bool test)
        {
            var ev = new Dictionary<string, object>();
            ev["event"] = "user-input";
            ev["kind"] = kind;
            if (kind != "key") { ev["x"] = x; ev["y"] = y; }
            if (dist > 0) ev["distance"] = Math.Round(dist);
            if (test) ev["test"] = true;
            ev["t"] = Environment.TickCount;
            lock (pending) { if (pending.Count < 64) pending.Enqueue(ev); }
            signal.Set();
        }

        static IntPtr MouseHook(int code, IntPtr w, IntPtr l)
        {
            if (code >= 0)
            {
                var s = (Native.MSLLHOOKSTRUCT)Marshal.PtrToStructure(l, typeof(Native.MSLLHOOKSTRUCT));
                // LLMHF_INJECTED and LLMHF_LOWER_IL_INJECTED.
                if ((s.flags & 0x3) == 0)
                {
                    int msg = w.ToInt32();
                    long now = Environment.TickCount;
                    if (msg == 0x200)
                    {
                        if (lastX != int.MinValue)
                        {
                            double dx = s.pt.X - lastX, dy = s.pt.Y - lastY;
                            travelled += Math.Sqrt(dx * dx + dy * dy);
                        }
                        lastX = s.pt.X; lastY = s.pt.Y;
                        if (now - lastMove >= 100) { Emit("mouse-move", s.pt.X, s.pt.Y, travelled, false); travelled = 0; lastMove = now; }
                    }
                    else if (now - lastButton >= 50)
                    {
                        Emit(msg == 0x20A || msg == 0x20E ? "wheel" : "mouse-button", s.pt.X, s.pt.Y, 0, false);
                        lastButton = now;
                    }
                }
            }
            return Native.CallNextHookEx(IntPtr.Zero, code, w, l);
        }

        static IntPtr KeyHook(int code, IntPtr w, IntPtr l)
        {
            if (code >= 0)
            {
                var s = (Native.KBDLLHOOKSTRUCT)Marshal.PtrToStructure(l, typeof(Native.KBDLLHOOKSTRUCT));
                int msg = w.ToInt32();
                // LLKHF_INJECTED and LLKHF_LOWER_IL_INJECTED; key downs only.
                if ((s.flags & 0x12) == 0 && (msg == 0x100 || msg == 0x104))
                {
                    long now = Environment.TickCount;
                    if (now - lastKey >= 100) { Emit("key", 0, 0, 0, false); lastKey = now; }
                }
            }
            return Native.CallNextHookEx(IntPtr.Zero, code, w, l);
        }
    }
}
