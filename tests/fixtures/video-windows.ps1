param([switch]$Resize)
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing @'
using System;
using System.Drawing;
using System.Windows.Forms;
public class VideoFixture : Form {
 public static bool ResizeTest;
 Timer timer = new Timer(); int frame;
 public VideoFixture() {
  Text=ResizeTest?"Kokoda Video Resize Test":"Kokoda Video Test"; ClientSize=ResizeTest?new Size(640,360):new Size(1280,720); DoubleBuffered=true;
  StartPosition=FormStartPosition.Manual;Location=new Point(50,50);
  timer.Interval=33;timer.Tick+=(s,e)=>{frame++;if(ResizeTest && frame%90==0)ClientSize=frame%180==0?new Size(640,360):new Size(800,480);Invalidate();};timer.Start();
 }
 protected override void OnPaint(PaintEventArgs e) {
  e.Graphics.Clear(Color.FromArgb(240,244,250));
  for(int x=0;x<1280;x+=80) e.Graphics.DrawLine(Pens.LightGray,x,0,x,720);
  for(int y=0;y<720;y+=80) e.Graphics.DrawLine(Pens.LightGray,0,y,1280,y);
  e.Graphics.FillRectangle(Brushes.RoyalBlue,(frame*8)%1000,160,240,180);
  e.Graphics.FillEllipse(Brushes.OrangeRed,1050-(frame*5)%1000,420,150,150);
  using(var font=new Font("Segoe UI",24)) e.Graphics.DrawString("Kokoda H.264 motion test - frame "+frame,font,Brushes.Black,30,35);
 }
 public static void Run(){Application.Run(new VideoFixture());}
}
'@
[VideoFixture]::ResizeTest=$Resize.IsPresent
[VideoFixture]::Run()
