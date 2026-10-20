package org.torqa.android;

import android.Manifest;
import android.app.Activity;
import android.app.PictureInPictureParams;
import android.content.Context;
import android.os.Build;
import android.util.Rational;
import android.view.View;

import org.godotengine.godot.Godot;
import org.godotengine.godot.plugin.GodotPlugin;
import org.godotengine.godot.plugin.UsedByGodot;

/**
 * Readies Torqa on Android (ADR 0024). Godot creates its plugins before it opens its
 * extensions, on the app's main thread: loading Torqa's library here runs its JNI_OnLoad with
 * the app's class loader, the one moment btleplug can find its Java classes. Then it hands the
 * app's context to HTTPS's certificate checks and asks for Bluetooth, which Android grants only
 * at run time.
 */
public class TorqaAndroid extends GodotPlugin {
    public TorqaAndroid(Godot godot) {
        super(godot);
        System.loadLibrary("torqa_gd");
    }

    /** Readies HTTPS's certificate checks through Android (rustls-platform-verifier). */
    private static native void initTls(Context context);

    @Override
    public String getPluginName() {
        return "TorqaAndroid";
    }

    @Override
    public View onMainCreate(Activity activity) {
        initTls(activity.getApplicationContext());
        String[] bluetooth = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
                ? new String[] {Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT}
                : new String[] {Manifest.permission.ACCESS_FINE_LOCATION};
        activity.requestPermissions(bluetooth, 1);
        return null;
    }

    /**
     * Shows Torqa in a small window on top of other apps, shaped like its content of `width` by
     * `height` (the ride's HUD, as the overlay on a computer, R55). Android allows shapes between
     * 1:2.39 and 2.39:1 only.
     */
    @UsedByGodot
    public void enterPictureInPicture(int width, int height) {
        Activity activity = getActivity();
        if (activity == null || Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
            return;
        }
        float limit = 2.39f;
        float shape = Math.max(1f / limit, Math.min(limit, (float) width / Math.max(height, 1)));
        Rational ratio = new Rational(Math.round(shape * 1000), 1000);
        PictureInPictureParams params =
                new PictureInPictureParams.Builder().setAspectRatio(ratio).build();
        activity.runOnUiThread(() -> activity.enterPictureInPictureMode(params));
    }

    /** Whether Torqa shows in the small window now; the rider may have opened it again. */
    @UsedByGodot
    public boolean isInPictureInPicture() {
        Activity activity = getActivity();
        return activity != null
                && Build.VERSION.SDK_INT >= Build.VERSION_CODES.N
                && activity.isInPictureInPictureMode();
    }
}
