Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing @'
using System;
using System.Drawing;
using System.Windows.Forms;
public class PopupMotion : Form {
 Timer timer=new Timer(); int frame;
 public PopupMotion(string title,Point position,bool alpha) {
  Text=title;ClientSize=new Size(640,360);StartPosition=FormStartPosition.Manual;
  Location=position;DoubleBuffered=true;
  if(alpha) {FormBorderStyle=FormBorderStyle.None;Opacity=0.65;}
  timer.Interval=33;timer.Tick+=(s,e)=>{frame++;Invalidate();};timer.Start();
 }
 protected override void OnPaint(PaintEventArgs e) {
  e.Graphics.Clear(Color.White);
  e.Graphics.FillRectangle(Brushes.RoyalBlue,(frame*8)%480,80,160,150);
  e.Graphics.FillEllipse(Brushes.OrangeRed,480-(frame*5)%480,220,90,90);
 }
 protected override void Dispose(bool disposing){if(disposing)timer.Dispose();base.Dispose(disposing);}
}
public class PopupVideoFixture : Form {
 public PopupVideoFixture() {
  Text="Kokoda Popup Video Root";ClientSize=new Size(320,180);
  StartPosition=FormStartPosition.Manual;Location=new Point(30,30);
  Shown+=(s,e)=>{
   new PopupMotion("Kokoda Popup Video Opaque",new Point(380,40),false).Show(this);
   new PopupMotion("Kokoda Popup Video Alpha",new Point(380,460),true).Show(this);
   var independent=new PopupMotion("Kokoda Popup Video Independent",new Point(1080,40),false);
   independent.Show();FormClosed+=(sender,args)=>independent.Close();
  };
 }
 public static void Run(){Application.Run(new PopupVideoFixture());}
}
'@
[PopupVideoFixture]::Run()
