#import <AppKit/AppKit.h>
#import <objc/runtime.h>

extern void pan_open_path(const char *path, int kind);

static void pan_receive_files(id self, SEL command, NSApplication *app, NSArray<NSString *> *files) {
    (void)self;
    (void)command;
    for (NSString *file in files) {
        pan_open_path(file.fileSystemRepresentation, 0);
    }
    [app replyToOpenOrPrint:NSApplicationDelegateReplySuccess];
}

static void pan_receive_urls(id self, SEL command, NSApplication *app, NSArray<NSURL *> *urls) {
    (void)self;
    (void)command;
    (void)app;
    for (NSURL *url in urls) {
        if (url.isFileURL && url.fileSystemRepresentation) {
            pan_open_path(url.fileSystemRepresentation, 0);
        }
    }
}

static void pan_attach_document_handlers(void) {
    id delegate = NSApp.delegate;
    if (!delegate) return;
    Class type = object_getClass(delegate);
    class_addMethod(type, @selector(application:openFiles:), (IMP)pan_receive_files, "v@:@@");
    class_addMethod(type, @selector(application:openURLs:), (IMP)pan_receive_urls, "v@:@@");
}

void pan_register_open_events(void) {
    [[NSNotificationCenter defaultCenter] addObserverForName:NSApplicationWillFinishLaunchingNotification
        object:nil queue:nil usingBlock:^(NSNotification *note) {
            (void)note;
            pan_attach_document_handlers();
        }];
}


void pan_pick_file(int kind) {
    dispatch_async(dispatch_get_main_queue(), ^{
        NSOpenPanel *panel = [NSOpenPanel openPanel];
        panel.canChooseFiles = kind != 3;
        panel.canChooseDirectories = kind == 3;
        panel.allowsMultipleSelection = kind == 0 || kind == 2;
        panel.title = kind == 1 ? @"Abrir subtítulos" : kind == 2 ? @"Añadir a PAN" : kind == 3 ? @"Elegir ubicación de la biblioteca" : kind == 4 ? @"Elegir carátula" : @"Abrir archivo";
        [panel beginWithCompletionHandler:^(NSModalResponse response) {
            if (response != NSModalResponseOK) return;
            for (NSURL *url in panel.URLs) {
                if (url.fileSystemRepresentation) {
                    pan_open_path(url.fileSystemRepresentation, kind);
                }
            }
        }];
    });
}

extern void pan_toolbar_action(int index);

@interface PANToolbar : NSObject <NSToolbarDelegate>
@end

@implementation PANToolbar
- (NSArray<NSToolbarItemIdentifier> *)toolbarAllowedItemIdentifiers:(NSToolbar *)toolbar {
    return @[@"pan.logo", NSToolbarFlexibleSpaceItemIdentifier, @"pan.open", @"pan.url", @"pan.library", @"pan.queue"];
}

- (NSArray<NSToolbarItemIdentifier> *)toolbarDefaultItemIdentifiers:(NSToolbar *)toolbar {
    return [self toolbarAllowedItemIdentifiers:toolbar];
}

- (NSToolbarItem *)toolbar:(NSToolbar *)toolbar itemForItemIdentifier:(NSToolbarItemIdentifier)identifier
    willBeInsertedIntoToolbar:(BOOL)inserted {
    NSToolbarItem *item = [[NSToolbarItem alloc] initWithItemIdentifier:identifier];
    if ([identifier isEqualToString:@"pan.logo"]) {
        item.label = @"PAN";
        item.bordered = NO;
        NSImageView *image = [NSImageView imageViewWithImage:NSApp.applicationIconImage];
        [image.widthAnchor constraintEqualToConstant:29].active = YES;
        [image.heightAnchor constraintEqualToConstant:29].active = YES;
        item.view = image;
        return item;
    }

    NSString *symbol;
    if ([identifier isEqualToString:@"pan.open"]) {
        item.label = @"Abrir archivo";
        symbol = @"folder";
    } else if ([identifier isEqualToString:@"pan.url"]) {
        item.label = @"Abrir URL";
        symbol = @"link";
    } else if ([identifier isEqualToString:@"pan.library"]) {
        item.label = @"Biblioteca";
        symbol = @"square.grid.2x2";
    } else if ([identifier isEqualToString:@"pan.queue"]) {
        item.label = @"Cola";
        symbol = @"list.bullet";
    } else {
        return nil;
    }
    item.image = [NSImage imageWithSystemSymbolName:symbol accessibilityDescription:item.label];
    item.toolTip = item.label;
    item.target = self;
    item.action = @selector(activate:);
    item.bordered = YES;
    return item;
}

- (void)activate:(NSToolbarItem *)sender {
    if ([sender.itemIdentifier isEqualToString:@"pan.open"]) {
        pan_pick_file(0);
    } else {
        int action = 2;
        if ([sender.itemIdentifier isEqualToString:@"pan.url"]) action = 1;
        if ([sender.itemIdentifier isEqualToString:@"pan.library"]) action = 3;
        pan_toolbar_action(action);
    }
}
@end

int pan_install_glass_toolbar(void *viewPointer) {
    if (@available(macOS 26.0, *)) {
        pan_attach_document_handlers();
        NSView *view = (__bridge NSView *)viewPointer;
        NSWindow *window = view.window;
        if (!window) return 0;
        PANToolbar *delegate = [PANToolbar new];
        NSToolbar *toolbar = [[NSToolbar alloc] initWithIdentifier:@"pan.playback"];
        toolbar.delegate = delegate;
        toolbar.displayMode = NSToolbarDisplayModeIconOnly;
        window.toolbarStyle = NSWindowToolbarStyleUnified;
        window.toolbar = toolbar;
        objc_setAssociatedObject(window, @selector(toolbarDefaultItemIdentifiers:),
            delegate, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        return 1;
    }
    return 0;
}
