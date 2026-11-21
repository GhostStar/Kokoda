param([Parameter(Mandatory=$true)][string]$StateFile)
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing @'
using System;using System.Drawing;using System.IO;using System.Runtime.InteropServices;using System.Windows.Forms;
public class DragFixture : Form {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int L,T,R,B;}
 [StructLayout(LayoutKind.Sequential)] public struct Pt {public int X,Y;}
 [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr value);
 [DllImport("user32.dll")] static extern bool GetCursorPos(out Pt p);
 [DllImport("user32.dll")] static extern short GetAsyncKeyState(int key);
 [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h,int attribute,out Rect r,int size);
 public static string StatePath;
 DragFixture child;TextBox text;Timer timer=new Timer();bool custom;
 public DragFixture(bool borderless) {
  custom=borderless;Text=custom?"Kokoda Drag Custom":"Kokoda Drag Root";
  AutoScaleMode=AutoScaleMode.None;ClientSize=new Size(500,300);
  StartPosition=FormStartPosition.Manual;Location=custom?new Point(700,220):new Point(80,80);
  if(custom)FormBorderStyle=FormBorderStyle.None;
  Controls.Add(new Label{Text="Synthetic window drag area",Location=new Point(15,5),Size=new Size(350,30),Enabled=false});
  text=new TextBox{Location=new Point(20,100),Size=new Size(450,40),Text="Synthetic drag selection text 0123456789 abcdefghijklmnopqrstuvwxyz",Font=new Font("Segoe UI",16)};Controls.Add(text);
  if(!custom){Shown+=(s,e)=>{child=new DragFixture(true);child.Show(this);timer.Start();};timer.Interval=20;timer.Tick+=(s,e)=>Publish();}
 }
 protected override void WndProc(ref Message m){
  if(custom && m.Msg==0x84){var p=PointToClient(new Point((short)((long)m.LParam&65535),(short)(((long)m.LParam>>16)&65535)));if(p.Y>=0&&p.Y<40){m.Result=(IntPtr)2;return;}}
  base.WndProc(ref m);
 }
 string Snapshot(){
  Rect r;DwmGetWindowAttribute(Handle,9,out r,Marshal.SizeOf(typeof(Rect)));
  var p=text.PointToScreen(new Point(8,text.Height/2));
  return "{\"id\":"+Handle.ToInt64()+",\"left\":"+r.L+",\"top\":"+r.T+",\"width\":"+(r.R-r.L)+",\"height\":"+(r.B-r.T)+",\"caption_x\":"+(r.L+150)+",\"caption_y\":"+(r.T+(custom?15:20))+",\"text_x\":"+p.X+",\"text_y\":"+p.Y+",\"selection\":"+text.SelectionLength+"}";
 }
 void Publish(){
  if(child==null||child.IsDisposed)return;Pt p;GetCursorPos(out p);
  try {File.WriteAllText(StatePath,"{\"root\":"+Snapshot()+",\"custom\":"+child.Snapshot()+",\"cursor_x\":"+p.X+",\"cursor_y\":"+p.Y+",\"left_down\":"+((GetAsyncKeyState(1)&32768)!=0?"true":"false")+"}");}catch(IOException){}
 }
 public static void Run(){SetProcessDpiAwarenessContext((IntPtr)(-4));Application.Run(new DragFixture(false));}
}
'@
[DragFixture]::StatePath=$StateFile
[DragFixture]::Run()
